//! Create PostgreSQL instance orchestration

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};

use uuid::Uuid;
use crate::orchestrations::{delete_instance, instance_actor};
use crate::types::{CreateInstanceInput, CreateInstanceOutput, DeleteInstanceInput, InstanceActorInput};
use crate::activities::{self, cms};
use std::time::Duration;
use crate::activity_types::{
    DeployPostgresOutput,
    DeployPostgresV2Input,
    WaitForReadyInput, WaitForReadyOutput,
    GetConnectionStringsInput, GetConnectionStringsOutput,
    TestConnectionInput, TestConnectionOutput,
    CreateInstanceRecordInput, CreateInstanceRecordOutput,
    UpdateInstanceStateInput, UpdateInstanceStateOutput,
    FreeDnsNameInput, FreeDnsNameOutput,
    RecordInstanceActorInput, RecordInstanceActorOutput,
    ImageType, ImageOperation, ImageOperationResult,
    SetInstanceRuntimeImageInput, SetInstanceRuntimeImageOutput,
    WaitForJobInput, WaitForJobOutput, DeleteJobInput, DeleteJobOutput,
};
use crate::activities::create_pvc::{CreatePvcInput, CreatePvcOutput};
use crate::activities::run_restore_job::{RunRestoreJobInput, RunRestoreJobOutput};
use crate::activities::deploy_postgres_from_pvc::{DeployPostgresFromPvcInput, DeployPostgresFromPvcOutput};

// ============================================================================
// v1.0.5 - Unified orchestration with inlined restore logic
// ============================================================================

/// v1.0.5: Creates PostgreSQL instance with full feature support.
///
/// Features:
/// - Normal creation with runtime image override support
/// - Restore from backup image (inlined, no delegation to older versions)
pub async fn create_instance_1_0_5_orchestration(
    ctx: OrchestrationContext,
    input: CreateInstanceInput,
) -> Result<CreateInstanceOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.5] Creating PostgreSQL instance: {} (user: {}, orchestration: {}, source_image: {:?}, runtime_image: {:?})",
        input.name, input.user_name, input.orchestration_id, input.source_image_id, input.runtime_image_id
    ));

    // Validate mutually exclusive options
    if input.source_image_id.is_some() && (input.runtime_image_id.is_some() || input.image_override.is_some()) {
        return Err("runtime_image_id/image_override cannot be used with source_image_id restore".to_string());
    }

    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    let postgres_version = input.postgres_version.clone().unwrap_or_else(|| "18".to_string());
    let storage_size_gb = input.storage_size_gb.unwrap_or(10);
    let use_load_balancer = input.use_load_balancer.unwrap_or(true);

    // If restoring from image, we need to fetch image details first
    let (effective_image_type, effective_postgres_version) = if let Some(ref image_id) = input.source_image_id {
        ctx.trace_info(format!("[v1.0.5] Fetching source image details: {}", image_id));
        let image_uuid = Uuid::parse_str(image_id)
            .map_err(|e| format!("Invalid image ID format: {}", e))?;

        let image_result = ctx
            .schedule_activity_typed::<ImageOperation, ImageOperationResult>(
                cms::image_ops::NAME,
                &ImageOperation::GetById { id: image_uuid },
            )
            .await?;

        match image_result {
            ImageOperationResult::Found { record } => {
                let img_type = match record.image_type.as_str() {
                    "pg_durable" => ImageType::PgDurable,
                    _ => ImageType::Stock,
                };
                ctx.trace_info(format!(
                    "[v1.0.5] Source image: {} (type: {}, postgres: {})",
                    record.name, record.image_type, record.postgres_version
                ));
                (img_type, record.postgres_version)
            }
            ImageOperationResult::NotFound => {
                return Err(format!("Source image not found: {}", image_id));
            }
            other => {
                return Err(format!("Unexpected result from image lookup: {:?}", other));
            }
        }
    } else {
        (input.image_type.clone(), postgres_version.clone())
    };

    // Reserve CMS record + DNS name
    let cms_input = CreateInstanceRecordInput {
        user_name: input.user_name.clone(),
        k8s_name: input.name.clone(),
        namespace: namespace.clone(),
        postgres_version: effective_postgres_version.clone(),
        storage_size_gb,
        use_load_balancer,
        dns_name: input.dns_label.clone(),
        orchestration_id: input.orchestration_id.clone(),
        image_type: effective_image_type.clone(),
    };

    ctx.schedule_activity_typed::<CreateInstanceRecordInput, CreateInstanceRecordOutput>(
        cms::create_instance_record::NAME,
        &cms_input,
    )
    .await?;

    // Record runtime_image_id if provided (only for normal creation path)
    if let Some(runtime_image_id) = &input.runtime_image_id {
        let image_uuid = Uuid::parse_str(runtime_image_id)
            .map_err(|e| format!("Invalid runtime_image_id: {}", e))?;

        let _ = ctx
            .schedule_activity_typed::<SetInstanceRuntimeImageInput, SetInstanceRuntimeImageOutput>(
                cms::set_instance_runtime_image::NAME,
                &SetInstanceRuntimeImageInput {
                    k8s_name: input.name.clone(),
                    runtime_image_id: image_uuid,
                },
            )
            .await?;
    }

    // Choose creation path
    let result = if let Some(ref image_id) = input.source_image_id {
        ctx.trace_info(format!("[v1.0.5] Restoring from image: {}", image_id));
        create_from_image_impl(
            &ctx,
            &input,
            &namespace,
            storage_size_gb,
            use_load_balancer,
            image_id,
        ).await
    } else {
        create_fresh_impl(
            &ctx,
            &input,
            &namespace,
            &effective_postgres_version,
            storage_size_gb,
            use_load_balancer,
            &effective_image_type,
        ).await
    };

    match result {
        Ok(output) => {
            ctx.trace_info("[v1.0.5] Instance created successfully");
            let update_input = UpdateInstanceStateInput {
                k8s_name: input.name.clone(),
                state: "running".to_string(),
                ip_connection_string: Some(output.ip_connection_string.clone()),
                dns_connection_string: output.dns_connection_string.clone(),
                external_ip: output.external_ip.clone(),
                delete_orchestration_id: None,
                message: Some(format!("Instance ready in {} seconds", output.deployment_time_seconds)),
            };
            update_cms_state(&ctx, update_input).await?;

            start_instance_actor(&ctx, &input.name, &namespace).await;
            Ok(output)
        }
        Err(e) => {
            ctx.trace_error(format!("[v1.0.5] Failed to create instance: {}", e));
            mark_instance_failed(&ctx, &input.name, &e).await;
            ctx.trace_info("[v1.0.5] Cleaning up partial deployment");

            if let Err(cleanup_err) = cleanup_on_failure(&ctx, &namespace, &input.name).await {
                ctx.trace_warn(format!("[v1.0.5] Cleanup failed: {}", cleanup_err));
            }

            Err(e)
        }
    }
}

// ============================================================================
// Fresh instance creation (no restore)
// ============================================================================

async fn create_fresh_impl(
    ctx: &OrchestrationContext,
    input: &CreateInstanceInput,
    namespace: &str,
    postgres_version: &str,
    storage_size_gb: i32,
    use_load_balancer: bool,
    image_type: &ImageType,
) -> Result<CreateInstanceOutput, String> {
    let start_time = ctx
        .utc_now()
        .await
        .map_err(|e| format!("Failed to get start time: {}", e))?;

    let effective_password = input.password.clone();

    ctx.trace_info(format!(
        "[v1.0.5] Step 1: Deploying PostgreSQL to Kubernetes (image: {})",
        image_type.as_str()
    ));

    let deploy_input = DeployPostgresV2Input {
        namespace: namespace.to_string(),
        instance_name: input.name.clone(),
        password: effective_password.clone(),
        postgres_version: postgres_version.to_string(),
        storage_size_gb,
        use_load_balancer,
        dns_label: input.dns_label.clone(),
        image_type: image_type.clone(),
        image_registry: None,
        image_override: input.image_override.clone(),
    };

    let _deploy_output = ctx
        .schedule_activity_typed::<DeployPostgresV2Input, DeployPostgresOutput>(
            activities::deploy_postgres_v2::NAME,
            &deploy_input,
        )
        .await?;

    // Step 2: Wait for pod ready
    ctx.trace_info("[v1.0.5] Step 2: Waiting for pod to be ready");
    wait_for_pod_ready(ctx, namespace, &input.name, "[v1.0.5]").await?;

    let end_time = ctx
        .utc_now()
        .await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let deployment_time = end_time
        .duration_since(start_time)
        .map_err(|e| format!("Failed to calculate duration: {}", e))?
        .as_secs();

    // Step 3: Get connection strings
    ctx.trace_info("[v1.0.5] Step 3: Getting connection strings");
    let conn_output = get_connection_strings(ctx, namespace, &input.name, &effective_password, use_load_balancer, &input.dns_label, image_type).await?;

    // Step 4: Test connection
    ctx.trace_info("[v1.0.5] Step 4: Testing PostgreSQL connection");
    let test_output = test_connection(ctx, namespace, &input.name, &effective_password, &conn_output).await?;

    build_output(input, namespace, &effective_password, &conn_output, &test_output, deployment_time)
}

// ============================================================================
// Restore from image
// ============================================================================

async fn create_from_image_impl(
    ctx: &OrchestrationContext,
    input: &CreateInstanceInput,
    namespace: &str,
    storage_size_gb: i32,
    use_load_balancer: bool,
    image_id: &str,
) -> Result<CreateInstanceOutput, String> {
    let start_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;

    // Step 1: Fetch image details from CMS
    ctx.trace_info(format!("[v1.0.5] Step 1: Fetching image details for {}", image_id));
    let image_uuid = Uuid::parse_str(image_id)
        .map_err(|e| format!("Invalid image ID format: {}", e))?;

    let image_result = ctx
        .schedule_activity_typed::<ImageOperation, ImageOperationResult>(
            cms::image_ops::NAME,
            &ImageOperation::GetById { id: image_uuid },
        )
        .await?;

    let image = match image_result {
        ImageOperationResult::Found { record } => record,
        ImageOperationResult::NotFound => {
            return Err(format!("Image not found: {}", image_id));
        }
        other => {
            return Err(format!("Unexpected result from image lookup: {:?}", other));
        }
    };

    if image.state != "ready" {
        return Err(format!("Image is not ready (state: {}). Cannot restore from non-ready image.", image.state));
    }

    ctx.trace_info(format!(
        "[v1.0.5] Found image: {} (postgres: {}, size: {} bytes)",
        image.name,
        image.postgres_version,
        image.backup_size_bytes.unwrap_or(0)
    ));

    // Get source password from image record
    ctx.trace_info("[v1.0.5] Fetching source password from image record");
    let password_result = ctx
        .schedule_activity_typed::<ImageOperation, ImageOperationResult>(
            cms::image_ops::NAME,
            &ImageOperation::GetSourcePassword { id: image_uuid },
        )
        .await?;

    let effective_password = match password_result {
        ImageOperationResult::PasswordFound { password } => {
            ctx.trace_info("[v1.0.5] Source password retrieved from image record");
            password
        }
        ImageOperationResult::NotFound => {
            return Err("Image password not found".to_string());
        }
        other => {
            return Err(format!("Unexpected result from password lookup: {:?}", other));
        }
    };

    let postgres_version = image.postgres_version.clone();
    let image_storage_size = image.storage_size_gb;

    // Step 2: Create empty PVC
    ctx.trace_info("[v1.0.5] Step 2: Creating PVC for restore target");
    let pvc_input = CreatePvcInput {
        name: input.name.clone(),
        namespace: namespace.to_string(),
        storage_size_gb: std::cmp::max(storage_size_gb as u32, image_storage_size as u32),
    };

    let pvc_output = ctx
        .schedule_activity_typed::<CreatePvcInput, CreatePvcOutput>(
            activities::create_pvc::NAME,
            &pvc_input,
        )
        .await?;

    let pvc_name = pvc_output.pvc_name;
    ctx.trace_info(format!("[v1.0.5] PVC created: {}", pvc_name));

    // Step 3: Run restore job
    ctx.trace_info("[v1.0.5] Step 3: Running restore job");
    let job_name = format!("restore-{}-{}", input.name, &input.orchestration_id[..8.min(input.orchestration_id.len())]);

    let restore_input = RunRestoreJobInput {
        job_name: job_name.clone(),
        namespace: namespace.to_string(),
        instance_name: input.name.clone(),
        pvc_name: pvc_name.clone(),
        postgres_version: postgres_version.clone(),
        blob_storage_account: extract_storage_account(&image.blob_storage_url)?,
        blob_container: image.blob_container.clone(),
        blob_path: image.blob_path.clone(),
    };

    ctx.schedule_activity_typed::<RunRestoreJobInput, RunRestoreJobOutput>(
        activities::run_restore_job::NAME,
        &restore_input,
    )
    .await?;

    ctx.trace_info("[v1.0.5] Restore job created, waiting for completion");

    // Step 4: Wait for restore job to complete
    let max_job_attempts = 120; // 20 minutes
    for attempt in 1..=max_job_attempts {
        let job_status = ctx
            .schedule_activity_typed::<WaitForJobInput, WaitForJobOutput>(
                activities::wait_for_job::NAME,
                &WaitForJobInput {
                    job_name: job_name.clone(),
                    namespace: namespace.to_string(),
                },
            )
            .await
            .map_err(|e| format!("Failed to check restore job status: {}", e))?;

        if job_status.succeeded {
            ctx.trace_info("[v1.0.5] Restore job completed successfully");
            break;
        } else if job_status.failed {
            return Err(format!("Restore job failed after {} attempts", attempt));
        }

        if attempt >= max_job_attempts {
            return Err(format!("Restore job timed out after {} attempts", max_job_attempts));
        }

        ctx.trace_info(format!("[v1.0.5] Restore job still running (attempt {}/{})", attempt, max_job_attempts));
        ctx.schedule_timer(Duration::from_secs(10)).await;
    }

    // Cleanup the restore job
    let _ = ctx
        .schedule_activity_typed::<DeleteJobInput, DeleteJobOutput>(
            activities::delete_job::NAME,
            &DeleteJobInput {
                job_name: job_name.clone(),
                namespace: namespace.to_string(),
                delete_secret: false,
            },
        )
        .await;

    // Step 5: Deploy StatefulSet and Service using existing PVC
    ctx.trace_info("[v1.0.5] Step 5: Deploying StatefulSet and Service");

    let image_type = match image.image_type.as_str() {
        "stock" => ImageType::Stock,
        "pg_durable" => ImageType::PgDurable,
        _ => ImageType::Stock,
    };

    let deploy_input = DeployPostgresFromPvcInput {
        instance_name: input.name.clone(),
        namespace: namespace.to_string(),
        postgres_version: postgres_version.clone(),
        password: effective_password.clone(),
        use_load_balancer,
        dns_label: input.dns_label.clone(),
        image_type: image_type.clone(),
        image_registry: None,
        pvc_name,
    };

    ctx.schedule_activity_typed::<DeployPostgresFromPvcInput, DeployPostgresFromPvcOutput>(
        activities::deploy_postgres_from_pvc::NAME,
        &deploy_input,
    )
    .await?;

    ctx.trace_info("[v1.0.5] StatefulSet and Service created");

    // Step 6: Wait for pod to be ready
    ctx.trace_info("[v1.0.5] Step 6: Waiting for pod to be ready");
    wait_for_pod_ready(ctx, namespace, &input.name, "[v1.0.5]").await?;

    let end_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let deployment_time = end_time.duration_since(start_time)
        .map_err(|e| format!("Failed to calculate duration: {}", e))?
        .as_secs();

    // Step 7: Get connection strings
    ctx.trace_info("[v1.0.5] Step 7: Getting connection strings");
    let conn_output = get_connection_strings(ctx, namespace, &input.name, &effective_password, use_load_balancer, &input.dns_label, &image_type).await?;

    // Step 8: Test connection
    ctx.trace_info("[v1.0.5] Step 8: Testing PostgreSQL connection");
    let test_output = test_connection(ctx, namespace, &input.name, &effective_password, &conn_output).await?;

    ctx.trace_info(format!("[v1.0.5] PostgreSQL version: {} (restored from image)", test_output.version));

    build_output(input, namespace, &effective_password, &conn_output, &test_output, deployment_time)
}

// ============================================================================
// Shared helpers
// ============================================================================

async fn wait_for_pod_ready(
    ctx: &OrchestrationContext,
    namespace: &str,
    instance_name: &str,
    version_tag: &str,
) -> Result<(), String> {
    let max_attempts = 60;

    for attempt in 1..=max_attempts {
        let wait_input = WaitForReadyInput {
            namespace: namespace.to_string(),
            instance_name: instance_name.to_string(),
            timeout_seconds: 0,
        };

        let wait_output = ctx
            .schedule_activity_typed::<WaitForReadyInput, WaitForReadyOutput>(
                activities::wait_for_ready::NAME,
                &wait_input,
            )
            .await
            .map_err(|e| format!("Failed to check pod status: {}", e))?;

        if wait_output.is_ready {
            return Ok(());
        }

        if attempt >= max_attempts {
            return Err(format!(
                "Timeout: Pod still in phase '{}' after {} attempts",
                wait_output.pod_phase, max_attempts
            ));
        }

        ctx.trace_info(format!(
            "{} Pod in phase '{}', waiting 5 seconds... (attempt {}/{})",
            version_tag, wait_output.pod_phase, attempt, max_attempts
        ));
        ctx.schedule_timer(Duration::from_secs(5)).await;
    }

    Ok(())
}

async fn get_connection_strings(
    ctx: &OrchestrationContext,
    namespace: &str,
    instance_name: &str,
    password: &str,
    use_load_balancer: bool,
    dns_label: &Option<String>,
    image_type: &ImageType,
) -> Result<GetConnectionStringsOutput, String> {
    let conn_input = GetConnectionStringsInput {
        namespace: namespace.to_string(),
        instance_name: instance_name.to_string(),
        password: password.to_string(),
        use_load_balancer,
        dns_label: dns_label.clone(),
        image_type: image_type.clone(),
    };

    ctx
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
        .await
}

async fn test_connection(
    ctx: &OrchestrationContext,
    namespace: &str,
    instance_name: &str,
    password: &str,
    conn_output: &GetConnectionStringsOutput,
) -> Result<TestConnectionOutput, String> {
    let test_connection_string = if let Some(dns) = &conn_output.dns_name {
        format!("postgresql://postgres:{}@{}:5432/postgres", password, dns)
    } else if let Some(ip) = &conn_output.external_ip {
        format!("postgresql://postgres:{}@{}:5432/postgres", password, ip)
    } else {
        let internal_host = format!("{}-svc.{}.svc.cluster.local", instance_name, namespace);
        format!("postgresql://postgres:{}@{}:5432/postgres", password, internal_host)
    };

    ctx
        .schedule_activity_with_retry_typed::<TestConnectionInput, TestConnectionOutput>(
            activities::test_connection::NAME,
            &TestConnectionInput {
                connection_string: test_connection_string,
                k8s_name: None,
            },
            RetryPolicy::new(5)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(2),
                    multiplier: 2.0,
                    max: Duration::from_secs(30),
                })
                .with_timeout(Duration::from_secs(120)),
        )
        .await
}

fn build_output(
    input: &CreateInstanceInput,
    namespace: &str,
    password: &str,
    conn_output: &GetConnectionStringsOutput,
    test_output: &TestConnectionOutput,
    deployment_time: u64,
) -> Result<CreateInstanceOutput, String> {
    let final_ip_connection_string = if let Some(ip) = &conn_output.external_ip {
        format!("postgresql://postgres:{}@{}:5432/postgres", password, ip)
    } else {
        let internal_host = format!("{}-svc.{}.svc.cluster.local", input.name, namespace);
        format!("postgresql://postgres:{}@{}:5432/postgres", password, internal_host)
    };

    let final_dns_connection_string = conn_output.dns_name.as_ref().map(|dns| {
        format!("postgresql://postgres:{}@{}:5432/postgres", password, dns)
    });

    Ok(CreateInstanceOutput {
        instance_name: input.name.clone(),
        namespace: namespace.to_string(),
        ip_connection_string: final_ip_connection_string,
        dns_connection_string: final_dns_connection_string,
        external_ip: conn_output.external_ip.clone(),
        dns_name: conn_output.dns_name.clone(),
        postgres_version: test_output.version.clone(),
        deployment_time_seconds: deployment_time,
    })
}

async fn cleanup_on_failure(
    ctx: &OrchestrationContext,
    namespace: &str,
    instance_name: &str,
) -> Result<(), String> {
    ctx.trace_info("Executing cleanup via delete-instance sub-orchestration");

    let delete_input = DeleteInstanceInput {
        name: instance_name.to_string(),
        namespace: Some(namespace.to_string()),
        orchestration_id: format!("cleanup-{}", instance_name),
    };

    let delete_output = ctx
        .schedule_sub_orchestration_typed::<DeleteInstanceInput, crate::types::DeleteInstanceOutput>(
            delete_instance::NAME,
            &delete_input
        )
        .await
        .map_err(|e| format!("Cleanup sub-orchestration failed: {}", e))?;

    if delete_output.deleted {
        ctx.trace_info("Resources cleaned up successfully via sub-orchestration");
    } else {
        ctx.trace_info("No resources found to clean up");
    }

    Ok(())
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
        last_query_result: None,
    };

    let input_json = serde_json::to_string(&actor_input)
        .unwrap_or_else(|_| "{}".to_string());

    ctx.schedule_orchestration(
        instance_actor::NAME,
        &actor_id,
        input_json,
    );

    ctx.trace_info(format!("Instance actor scheduled: {}", actor_id));

    if let Err(err) = ctx
        .schedule_activity_typed::<RecordInstanceActorInput, RecordInstanceActorOutput>(
            cms::record_instance_actor::NAME,
            &RecordInstanceActorInput {
                k8s_name: k8s_name.to_string(),
                instance_actor_orchestration_id: actor_id,
            },
        )
        .await
    {
        ctx.trace_warn(format!("Failed to record instance actor ID: {}", err));
    }
}

async fn update_cms_state(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) -> Result<(), String> {
    ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .await
        .map_err(|e| format!("Failed to update CMS state: {}", e))?;
    Ok(())
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
    if let Err(e) = update_cms_state(ctx, update_input).await {
        ctx.trace_warn(format!("[v1.0.5] Failed to mark instance as failed in CMS: {}", e));
    }

    if let Err(err) = ctx
        .schedule_activity_typed::<FreeDnsNameInput, FreeDnsNameOutput>(
            cms::free_dns_name::NAME,
            &FreeDnsNameInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .await
    {
        ctx.trace_warn(format!("[v1.0.5] Failed to free DNS name: {}", err));
    }
}

fn extract_storage_account(blob_url: &str) -> Result<String, String> {
    let url = url::Url::parse(blob_url)
        .map_err(|e| format!("Invalid blob URL: {}", e))?;

    let host = url.host_str()
        .ok_or_else(|| "Blob URL missing host".to_string())?;

    let account = host.split('.').next()
        .ok_or_else(|| "Invalid blob URL format".to_string())?;

    Ok(account.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_types::ImageType;

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
            source_image_id: None,
            runtime_image_id: None,
            image_override: None,
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
