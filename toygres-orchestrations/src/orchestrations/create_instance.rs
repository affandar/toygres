//! Create PostgreSQL instance orchestration

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};
use crate::names::orchestrations;
use crate::types::{CreateInstanceInput, CreateInstanceOutput, DeleteInstanceInput, InstanceActorInput};
use crate::activities::{self, cms};
use std::time::Duration;
use crate::activity_types::{
    DeployPostgresInput, DeployPostgresOutput,
    WaitForReadyInput, WaitForReadyOutput,
    GetConnectionStringsInput, GetConnectionStringsOutput,
    TestConnectionInput, TestConnectionOutput,
    CreateInstanceRecordInput, CreateInstanceRecordOutput,
    UpdateInstanceStateInput, UpdateInstanceStateOutput,
    FreeDnsNameInput, FreeDnsNameOutput,
    RecordInstanceActorInput, RecordInstanceActorOutput,
    ImageType,
};

pub async fn create_instance_orchestration(
    ctx: OrchestrationContext,
    input: CreateInstanceInput,
) -> Result<CreateInstanceOutput, String> {
    ctx.trace_info(format!(
        "Creating PostgreSQL instance: {} (user: {}, orchestration: {})",
        input.name, input.user_name, input.orchestration_id
    ));
    
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    let postgres_version = input.postgres_version.clone().unwrap_or_else(|| "18".to_string());
    let storage_size_gb = input.storage_size_gb.unwrap_or(10);
    let use_load_balancer = input.use_load_balancer.unwrap_or(true);
    
    // Reserve CMS record + DNS name
    let cms_input = CreateInstanceRecordInput {
        user_name: input.user_name.clone(),
        k8s_name: input.name.clone(),
        namespace: namespace.clone(),
        postgres_version: postgres_version.clone(),
        storage_size_gb,
        use_load_balancer,
        dns_name: input.dns_label.clone(),
        orchestration_id: input.orchestration_id.clone(),
        image_type: input.image_type.clone(),
    };
    
    ctx.schedule_activity_typed::<CreateInstanceRecordInput, CreateInstanceRecordOutput>(
            cms::create_instance_record::NAME,
            &cms_input,
        )
        .into_activity_typed::<CreateInstanceRecordOutput>()
        .await?;
    
    match create_instance_impl(&ctx, &input, &namespace, &postgres_version, storage_size_gb, use_load_balancer, &input.image_type).await {
        Ok(output) => {
            ctx.trace_info("Instance created successfully");
            let update_input = UpdateInstanceStateInput {
                k8s_name: input.name.clone(),
                state: "running".to_string(),
                ip_connection_string: Some(output.ip_connection_string.clone()),
                dns_connection_string: output.dns_connection_string.clone(),
                external_ip: output.external_ip.clone(),
                delete_orchestration_id: None,
                message: Some(format!("Instance ready in {} seconds", output.deployment_time_seconds)),
            };
            update_cms_state(&ctx, update_input).await;
            
            // Start instance actor (detached orchestration for continuous monitoring and per-instance tasks)
            start_instance_actor(&ctx, &input.name, &namespace).await;
            
            Ok(output)
        }
        Err(e) => {
            ctx.trace_error(format!("Failed to create instance: {}", e));
            mark_instance_failed(&ctx, &input.name, &e).await;
            ctx.trace_info("Cleaning up partial deployment");
            
            if let Err(cleanup_err) = cleanup_on_failure(&ctx, &namespace, &input.name).await {
                ctx.trace_warn(format!("Cleanup failed: {}", cleanup_err));
            } else {
                ctx.trace_info("Cleanup complete, system restored to original state");
            }
            
            Err(e)
        }
    }
}

async fn create_instance_impl(
    ctx: &OrchestrationContext,
    input: &CreateInstanceInput,
    namespace: &str,
    postgres_version: &str,
    storage_size_gb: i32,
    use_load_balancer: bool,
    image_type: &ImageType,
) -> Result<CreateInstanceOutput, String> {
    let start_time = ctx.utcnow().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;
    
    // The user's desired password - this is what we want the final password to be
    let effective_password = input.password.clone();
    
    // Step 1: Deploy PostgreSQL
    ctx.trace_info(format!("Step 1: Deploying PostgreSQL to Kubernetes (image: {})", image_type.as_str()));
    let deploy_input = DeployPostgresInput {
        namespace: namespace.to_string(),
        instance_name: input.name.clone(),
        password: effective_password.clone(),
        postgres_version: postgres_version.to_string(),
        storage_size_gb,
        use_load_balancer,
        dns_label: input.dns_label.clone(),
        image_type: image_type.clone(),
        image_registry: None, // Use default ACR
    };
    
    let _deploy_output = ctx
        .schedule_activity_typed::<DeployPostgresInput, DeployPostgresOutput>(activities::deploy_postgres::NAME, &deploy_input)
        .into_activity_typed::<DeployPostgresOutput>()
        .await?;
    
    ctx.trace_info("PostgreSQL resources created");
    
    // Step 2: Poll for pod to be ready (using Duroxide timers for determinism)
    ctx.trace_info("Step 2: Waiting for pod to be ready");
    let max_attempts = 60; // 5 minutes (60 attempts * 5 seconds)
    
    for attempt in 1..=max_attempts {
        // Check pod status
        let wait_input = WaitForReadyInput {
            namespace: namespace.to_string(),
            instance_name: input.name.clone(),
            timeout_seconds: 0, // No timeout in activity, just check current status
        };
        
        let wait_output = ctx
            .schedule_activity_typed::<WaitForReadyInput, WaitForReadyOutput>(activities::wait_for_ready::NAME, &wait_input)
            .into_activity_typed::<WaitForReadyOutput>()
            .await
            .map_err(|e| format!("Failed to check pod status: {}", e))?;
        
        // Check if pod is ready
        if wait_output.is_ready {
            let end_time = ctx.utcnow().await
                .map_err(|e| format!("Failed to get end time: {}", e))?;
            let elapsed = end_time.duration_since(start_time)
                .map_err(|e| format!("Failed to calculate duration: {}", e))?
                .as_secs();
            ctx.trace_info(format!("Pod ready (phase: {}, took {} seconds)", wait_output.pod_phase, elapsed));
            break;
        }
        
        // Pod not ready yet
        if attempt >= max_attempts {
            return Err(format!("Timeout: Pod still in phase '{}' after {} attempts", wait_output.pod_phase, max_attempts));
        }
        
        // Log status and wait before next check
        ctx.trace_info(format!("Pod in phase '{}', not ready yet (attempt {}/{}), waiting 5 seconds...", 
                               wait_output.pod_phase, attempt, max_attempts));
        
        // Wait 5 seconds using Duroxide timer (deterministic)
        ctx.schedule_timer(Duration::from_secs(5)).into_timer().await;
    }
    
    let end_time = ctx.utcnow().await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let deployment_time = end_time.duration_since(start_time)
        .map_err(|e| format!("Failed to calculate duration: {}", e))?
        .as_secs();
    
    // Step 3: Get connection strings
    // Both pg_durable and stock images correctly use POSTGRES_PASSWORD env var
    ctx.trace_info("Step 3: Getting connection strings");
    let conn_input = GetConnectionStringsInput {
        namespace: namespace.to_string(),
        instance_name: input.name.clone(),
        password: effective_password.clone(),
        use_load_balancer,
        dns_label: input.dns_label.clone(),
        image_type: image_type.clone(),
    };
    
    // Get connection strings with retry - Azure LoadBalancer IP assignment can be slow
    let conn_output = ctx
        .schedule_activity_with_retry_typed::<GetConnectionStringsInput, GetConnectionStringsOutput>(
            activities::get_connection_strings::NAME,
            &conn_input,
            RetryPolicy::new(5)
                .with_backoff(BackoffStrategy::Linear {
                    base: Duration::from_secs(2),
                    max: Duration::from_secs(10),
                })
                .with_timeout(Duration::from_secs(120)),
        )
        .await?;
    
    ctx.trace_info("Connection strings generated");
    
    // Step 4: Test connection
    // Use the final password (effective_password) for testing
    ctx.trace_info("Step 4: Testing PostgreSQL connection");
    
    // Build test connection string with the final password
    let test_connection_string = if let Some(dns) = &conn_output.dns_name {
        format!("postgresql://postgres:{}@{}:5432/postgres", effective_password, dns)
    } else if let Some(ip) = &conn_output.external_ip {
        format!("postgresql://postgres:{}@{}:5432/postgres", effective_password, ip)
    } else {
        // Fallback to internal connection with updated password
        let internal_host = format!("{}-svc.{}.svc.cluster.local", input.name, namespace);
        format!("postgresql://postgres:{}@{}:5432/postgres", effective_password, internal_host)
    };
    
    let test_input = TestConnectionInput {
        connection_string: test_connection_string,
    };
    
    // Test connection with retry - PostgreSQL might still be initializing
    let test_output = ctx
        .schedule_activity_with_retry_typed::<TestConnectionInput, TestConnectionOutput>(
            activities::test_connection::NAME,
            &test_input,
            RetryPolicy::new(5)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(2),
                    multiplier: 2.0,
                    max: Duration::from_secs(30),
                })
                .with_timeout(Duration::from_secs(60)),
        )
        .await?;
    
    ctx.trace_info(format!("PostgreSQL version: {}", test_output.version));
    
    // Build final connection strings with the effective password
    let final_ip_connection_string = if let Some(ip) = &conn_output.external_ip {
        format!("postgresql://postgres:{}@{}:5432/postgres", effective_password, ip)
    } else {
        let internal_host = format!("{}-svc.{}.svc.cluster.local", input.name, namespace);
        format!("postgresql://postgres:{}@{}:5432/postgres", effective_password, internal_host)
    };
    
    let final_dns_connection_string = conn_output.dns_name.as_ref().map(|dns| {
        format!("postgresql://postgres:{}@{}:5432/postgres", effective_password, dns)
    });
    
    // Build output
    Ok(CreateInstanceOutput {
        instance_name: input.name.clone(),
        namespace: namespace.to_string(),
        ip_connection_string: final_ip_connection_string,
        dns_connection_string: final_dns_connection_string,
        external_ip: conn_output.external_ip,
        dns_name: conn_output.dns_name,
        postgres_version: test_output.version,
        deployment_time_seconds: deployment_time,
    })
}

async fn cleanup_on_failure(
    ctx: &OrchestrationContext,
    namespace: &str,
    instance_name: &str,
) -> Result<(), String> {
    ctx.trace_info("Executing cleanup via delete-instance sub-orchestration");
    
    // Call DeleteInstanceOrchestration as a sub-orchestration
    // This reuses all the deletion logic and ensures consistency
    let delete_input = DeleteInstanceInput {
        name: instance_name.to_string(),
        namespace: Some(namespace.to_string()),
        orchestration_id: format!("cleanup-{}", instance_name),
    };
    
    let delete_output = ctx
        .schedule_sub_orchestration_typed::<DeleteInstanceInput, crate::types::DeleteInstanceOutput>(
            orchestrations::DELETE_INSTANCE,
            &delete_input
        )
        .into_sub_orchestration_typed::<crate::types::DeleteInstanceOutput>()
        .await
        .map_err(|e| format!("Cleanup sub-orchestration failed: {}", e))?;
    
    if delete_output.deleted {
        ctx.trace_info("Resources cleaned up successfully via sub-orchestration");
    } else {
        ctx.trace_info("No resources found to clean up");
    }
    
    Ok(())
}

async fn update_cms_state(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) {
    if let Err(err) = ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .into_activity_typed::<UpdateInstanceStateOutput>()
        .await
    {
        ctx.trace_warn(format!("Failed to update CMS state: {}", err));
    }
}

async fn start_instance_actor(
    ctx: &OrchestrationContext,
    k8s_name: &str,
    namespace: &str,
) {
    ctx.trace_info("Starting instance actor for continuous monitoring");
    
    let actor_id = format!("actor-{}", k8s_name);
    
    let actor_input = InstanceActorInput {
        k8s_name: k8s_name.to_string(),
        namespace: namespace.to_string(),
        orchestration_id: actor_id.clone(),
    };
    
    // Start as a detached orchestration (runs independently)
    let input_json = serde_json::to_string(&actor_input)
        .unwrap_or_else(|_| "{}".to_string());
    
    ctx.schedule_orchestration(
        orchestrations::INSTANCE_ACTOR,
        &actor_id,
        input_json,
    );
    
    ctx.trace_info(format!("Instance actor scheduled: {}", actor_id));
    
    // Record the actor orchestration ID in CMS
    if let Err(err) = ctx
        .schedule_activity_typed::<RecordInstanceActorInput, RecordInstanceActorOutput>(
            cms::record_instance_actor::NAME,
            &RecordInstanceActorInput {
                k8s_name: k8s_name.to_string(),
                instance_actor_orchestration_id: actor_id,
            },
        )
        .into_activity_typed::<RecordInstanceActorOutput>()
        .await
    {
        ctx.trace_warn(format!("Failed to record instance actor ID: {}", err));
    }
}

async fn mark_instance_failed(
    ctx: &OrchestrationContext,
    k8s_name: &str,
    error: &str,
) {
    let update_input = UpdateInstanceStateInput {
        k8s_name: k8s_name.to_string(),
        state: "failed".to_string(),
        ip_connection_string: None,
        dns_connection_string: None,
        external_ip: None,
        delete_orchestration_id: None,
        message: Some(error.to_string()),
    };
    update_cms_state(ctx, update_input).await;

    if let Err(err) = ctx
        .schedule_activity_typed::<FreeDnsNameInput, FreeDnsNameOutput>(
            cms::free_dns_name::NAME,
            &FreeDnsNameInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .into_activity_typed::<FreeDnsNameOutput>()
        .await
    {
        ctx.trace_warn(format!("Failed to free DNS name: {}", err));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_instance_input_serialization() {
        let input = CreateInstanceInput {
            user_name: "test".to_string(),
            name: "test-pg".to_string(),
            password: "pass123".to_string(),
            postgres_version: Some("18".to_string()),
            storage_size_gb: Some(10),
            use_load_balancer: Some(true),
            dns_label: Some("test".to_string()),
            namespace: Some("toygres".to_string()),
            orchestration_id: "create-test".to_string(),
            image_type: ImageType::Stock,
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: CreateInstanceInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_create_instance_output_serialization() {
        let output = CreateInstanceOutput {
            instance_name: "test-pg".to_string(),
            namespace: "toygres".to_string(),
            ip_connection_string: "postgresql://postgres:pass@1.2.3.4:5432/postgres".to_string(),
            dns_connection_string: Some("postgresql://postgres:pass@test.eastus.cloudapp.azure.com:5432/postgres".to_string()),
            external_ip: Some("1.2.3.4".to_string()),
            dns_name: Some("test.eastus.cloudapp.azure.com".to_string()),
            postgres_version: "PostgreSQL 18.0".to_string(),
            deployment_time_seconds: 45,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: CreateInstanceOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

// ============================================================================
// v1.0.1 - Propagate CMS update errors on success path
// ============================================================================

/// v1.0.1: Creates PostgreSQL instance with proper CMS error propagation
/// 
/// Changes from v1.0.0:
/// - CMS state update errors are now propagated on success path (fail orchestration if CMS fails)
/// - On failure path, CMS errors are still logged but don't prevent cleanup
pub async fn create_instance_1_0_1(
    ctx: OrchestrationContext,
    input: CreateInstanceInput,
) -> Result<CreateInstanceOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.1] Creating PostgreSQL instance: {} (user: {}, orchestration: {})",
        input.name, input.user_name, input.orchestration_id
    ));
    
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    let postgres_version = input.postgres_version.clone().unwrap_or_else(|| "18".to_string());
    let storage_size_gb = input.storage_size_gb.unwrap_or(10);
    let use_load_balancer = input.use_load_balancer.unwrap_or(true);
    
    // Reserve CMS record + DNS name
    let cms_input = CreateInstanceRecordInput {
        user_name: input.user_name.clone(),
        k8s_name: input.name.clone(),
        namespace: namespace.clone(),
        postgres_version: postgres_version.clone(),
        storage_size_gb,
        use_load_balancer,
        dns_name: input.dns_label.clone(),
        orchestration_id: input.orchestration_id.clone(),
        image_type: input.image_type.clone(),
    };
    
    ctx.schedule_activity_typed::<CreateInstanceRecordInput, CreateInstanceRecordOutput>(
            cms::create_instance_record::NAME,
            &cms_input,
        )
        .into_activity_typed::<CreateInstanceRecordOutput>()
        .await?;
    
    match create_instance_impl(&ctx, &input, &namespace, &postgres_version, storage_size_gb, use_load_balancer, &input.image_type).await {
        Ok(output) => {
            ctx.trace_info("[v1.0.1] Instance created successfully");
            let update_input = UpdateInstanceStateInput {
                k8s_name: input.name.clone(),
                state: "running".to_string(),
                ip_connection_string: Some(output.ip_connection_string.clone()),
                dns_connection_string: output.dns_connection_string.clone(),
                external_ip: output.external_ip.clone(),
                delete_orchestration_id: None,
                message: Some(format!("Instance ready in {} seconds", output.deployment_time_seconds)),
            };
            // v1.0.1: Propagate CMS update error on success path
            update_cms_state_v1_0_1(&ctx, update_input).await?;
            
            // Start instance actor (detached orchestration for continuous monitoring and per-instance tasks)
            start_instance_actor(&ctx, &input.name, &namespace).await;
            
            Ok(output)
        }
        Err(e) => {
            ctx.trace_error(format!("[v1.0.1] Failed to create instance: {}", e));
            mark_instance_failed_v1_0_1(&ctx, &input.name, &e).await;
            ctx.trace_info("[v1.0.1] Cleaning up partial deployment");
            
            if let Err(cleanup_err) = cleanup_on_failure(&ctx, &namespace, &input.name).await {
                ctx.trace_warn(format!("[v1.0.1] Cleanup failed: {}", cleanup_err));
            } else {
                ctx.trace_info("[v1.0.1] Cleanup complete, system restored to original state");
            }
            
            Err(e)
        }
    }
}

/// v1.0.1: CMS state update that returns Result
async fn update_cms_state_v1_0_1(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) -> Result<(), String> {
    ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .into_activity_typed::<UpdateInstanceStateOutput>()
        .await
        .map_err(|e| format!("Failed to update CMS state: {}", e))?;
    Ok(())
}

/// v1.0.1: Mark instance failed (best-effort, logs errors)
async fn mark_instance_failed_v1_0_1(
    ctx: &OrchestrationContext,
    k8s_name: &str,
    error: &str,
) {
    let update_input = UpdateInstanceStateInput {
        k8s_name: k8s_name.to_string(),
        state: "failed".to_string(),
        ip_connection_string: None,
        dns_connection_string: None,
        external_ip: None,
        delete_orchestration_id: None,
        message: Some(error.to_string()),
    };
    if let Err(e) = update_cms_state_v1_0_1(ctx, update_input).await {
        ctx.trace_warn(format!("[v1.0.1] Failed to mark instance as failed in CMS: {}", e));
    }

    if let Err(err) = ctx
        .schedule_activity_typed::<FreeDnsNameInput, FreeDnsNameOutput>(
            cms::free_dns_name::NAME,
            &FreeDnsNameInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .into_activity_typed::<FreeDnsNameOutput>()
        .await
    {
        ctx.trace_warn(format!("[v1.0.1] Failed to free DNS name: {}", err));
    }
}

