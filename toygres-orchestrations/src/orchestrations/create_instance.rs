//! Create PostgreSQL instance orchestration

use duroxide::OrchestrationContext;
use crate::names::orchestrations;
use crate::types::{CreateInstanceInput, CreateInstanceOutput, DeleteInstanceInput};
use crate::activity_names::activities;
use crate::activity_types::{
    DeployPostgresInput, DeployPostgresOutput,
    DeletePostgresInput, DeletePostgresOutput,
    WaitForReadyInput, WaitForReadyOutput,
    GetConnectionStringsInput, GetConnectionStringsOutput,
    TestConnectionInput, TestConnectionOutput,
};

pub async fn create_instance_orchestration(
    ctx: OrchestrationContext,
    input: CreateInstanceInput,
) -> Result<CreateInstanceOutput, String> {
    ctx.trace_info(format!("Creating PostgreSQL instance: {}", input.name));
    
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    let postgres_version = input.postgres_version.clone().unwrap_or_else(|| "18".to_string());
    let storage_size_gb = input.storage_size_gb.unwrap_or(10);
    let use_load_balancer = input.use_load_balancer.unwrap_or(true);
    
    // Execute orchestration with error handling and cleanup
    match create_instance_impl(&ctx, &input, &namespace, &postgres_version, storage_size_gb, use_load_balancer).await {
        Ok(output) => {
            ctx.trace_info("Instance created successfully");
            Ok(output)
        }
        Err(e) => {
            ctx.trace_error(format!("Failed to create instance: {}", e));
            ctx.trace_info("Cleaning up partial deployment");
            
            // Cleanup: delete any resources that were created
            let cleanup_result = cleanup_on_failure(&ctx, &namespace, &input.name).await;
            
            match cleanup_result {
                Ok(_) => {
                    ctx.trace_info("Cleanup complete, system restored to original state");
                }
                Err(cleanup_err) => {
                    ctx.trace_warn(format!("Cleanup failed: {}", cleanup_err));
                }
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
) -> Result<CreateInstanceOutput, String> {
    let start_time_ms = ctx.utcnow_ms().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;
    
    // Step 1: Deploy PostgreSQL
    ctx.trace_info("Step 1: Deploying PostgreSQL to Kubernetes");
    let deploy_input = DeployPostgresInput {
        namespace: namespace.to_string(),
        instance_name: input.name.clone(),
        password: input.password.clone(),
        postgres_version: postgres_version.to_string(),
        storage_size_gb,
        use_load_balancer,
        dns_label: input.dns_label.clone(),
    };
    
    let _deploy_output = ctx
        .schedule_activity_typed::<DeployPostgresInput, DeployPostgresOutput>(activities::DEPLOY_POSTGRES, &deploy_input)
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
            .schedule_activity_typed::<WaitForReadyInput, WaitForReadyOutput>(activities::WAIT_FOR_READY, &wait_input)
            .into_activity_typed::<WaitForReadyOutput>()
            .await
            .map_err(|e| format!("Failed to check pod status: {}", e))?;
        
        // Check if pod is ready
        if wait_output.is_ready {
            let end_time_ms = ctx.utcnow_ms().await
                .map_err(|e| format!("Failed to get end time: {}", e))?;
            let elapsed = (end_time_ms - start_time_ms) / 1000; // Convert ms to seconds
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
        ctx.schedule_timer(5000).into_timer().await; // 5000 milliseconds = 5 seconds
    }
    
    let end_time_ms = ctx.utcnow_ms().await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let deployment_time = (end_time_ms - start_time_ms) / 1000; // Convert ms to seconds
    
    // Step 3: Get connection strings
    ctx.trace_info("Step 3: Getting connection strings");
    let conn_input = GetConnectionStringsInput {
        namespace: namespace.to_string(),
        instance_name: input.name.clone(),
        password: input.password.clone(),
        use_load_balancer,
        dns_label: input.dns_label.clone(),
    };
    
    let conn_output = ctx
        .schedule_activity_typed::<GetConnectionStringsInput, GetConnectionStringsOutput>(activities::GET_CONNECTION_STRINGS, &conn_input)
        .into_activity_typed::<GetConnectionStringsOutput>()
        .await?;
    
    ctx.trace_info("Connection strings generated");
    
    // Step 4: Test connection
    ctx.trace_info("Step 4: Testing PostgreSQL connection");
    let test_connection_string = conn_output.dns_connection_string.clone()
        .unwrap_or_else(|| conn_output.ip_connection_string.clone());
    
    let test_input = TestConnectionInput {
        connection_string: test_connection_string,
    };
    
    let test_output = ctx
        .schedule_activity_typed::<TestConnectionInput, TestConnectionOutput>(activities::TEST_CONNECTION, &test_input)
        .into_activity_typed::<TestConnectionOutput>()
        .await?;
    
    ctx.trace_info(format!("PostgreSQL version: {}", test_output.version));
    
    // Build output
    Ok(CreateInstanceOutput {
        instance_name: input.name.clone(),
        namespace: namespace.to_string(),
        ip_connection_string: conn_output.ip_connection_string,
        dns_connection_string: conn_output.dns_connection_string,
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_instance_input_serialization() {
        let input = CreateInstanceInput {
            name: "test-pg".to_string(),
            password: "pass123".to_string(),
            postgres_version: Some("18".to_string()),
            storage_size_gb: Some(10),
            use_load_balancer: Some(true),
            dns_label: Some("test".to_string()),
            namespace: Some("toygres".to_string()),
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

