//! Create Image orchestration
//!
//! Creates a backup image from a running PostgreSQL instance using pg_basebackup
//! and stores it in Azure Blob Storage.

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::create-image";
use std::time::Duration;
use uuid::Uuid;

use crate::activities::{self, cms};
use crate::activity_types::{
    ImageOperation, ImageOperationResult, ImageState,
    RunBackupJobInput, RunBackupJobOutput,
    WaitForJobInput, WaitForJobOutput,
    DeleteJobInput, DeleteJobOutput,
    GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput,
    GetInstancePasswordInput, GetInstancePasswordOutput,
};
use crate::types::{CreateImageInput, CreateImageOutput};

pub async fn create_image_orchestration(
    ctx: OrchestrationContext,
    input: CreateImageInput,
) -> Result<CreateImageOutput, String> {
    let start_time = std::time::Instant::now();
    
    ctx.trace_info(format!(
        "Creating image '{}' from instance '{}'",
        input.name, input.source_k8s_name
    ));
    
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    
    // Step 1: Validate source instance exists and get its details
    ctx.trace_info("Step 1: Validating source instance");
    let instance = get_source_instance(&ctx, &input.source_k8s_name).await?;
    
    // Step 1b: Fetch password from K8s Secret if not provided
    let source_password = match &input.source_password {
        Some(pw) if !pw.is_empty() => {
            ctx.trace_info("Using provided password");
            pw.clone()
        }
        _ => {
            ctx.trace_info("Fetching password from K8s Secret");
            get_password_from_secret(&ctx, &input.source_k8s_name, &namespace).await?
        }
    };
    
    // Get storage configuration from environment (set in configmap)
    let blob_storage_account = std::env::var("AZURE_STORAGE_ACCOUNT")
        .unwrap_or_else(|_| "toygresstorage".to_string());
    let blob_container = std::env::var("AZURE_STORAGE_CONTAINER")
        .unwrap_or_else(|_| "toygres-images".to_string());
    
    // Generate unique blob path: images/{image-name}-{timestamp}/
    // IMPORTANT: Use ctx.utcnow() for deterministic replay, not chrono::Utc::now()
    let now = ctx.utcnow().await
        .map_err(|e| format!("Failed to get current time: {}", e))?;
    let timestamp = chrono::DateTime::<chrono::Utc>::from(now).format("%Y%m%d%H%M%S");
    let blob_path = format!("images/{}-{}", input.name, timestamp);
    let blob_storage_url = format!("https://{}.blob.core.windows.net", blob_storage_account);
    
    // Step 2: Create CMS record
    ctx.trace_info("Step 2: Creating CMS record");
    let image_id = create_cms_record(
        &ctx,
        &input,
        &instance,
        &blob_storage_url,
        &blob_container,
        &blob_path,
        &source_password,
    ).await?;
    
    // Step 3: Run backup job
    ctx.trace_info("Step 3: Starting backup job");
    // Sanitize name for K8s: lowercase, replace underscores with hyphens
    let sanitized_name = input.name.to_lowercase().replace('_', "-");
    // Use LAST 8 characters of orchestration_id for uniqueness (the UUID suffix)
    let orch_suffix = if input.orchestration_id.len() > 8 {
        &input.orchestration_id[input.orchestration_id.len() - 8..]
    } else {
        &input.orchestration_id
    };
    let job_name = format!("backup-{}-{}", sanitized_name, orch_suffix);
    
    match run_backup(&ctx, &input, &job_name, &namespace, &instance, &blob_storage_account, &blob_container, &blob_path, &source_password).await {
        Ok((backup_size, checksum)) => {
            // Step 4: Update CMS to ready
            ctx.trace_info("Step 4: Updating CMS to ready");
            update_image_state(&ctx, &input.name, ImageState::Ready, backup_size, checksum.clone(), None).await;
            
            // Step 5: Cleanup job
            ctx.trace_info("Step 5: Cleaning up backup job");
            cleanup_job(&ctx, &job_name, &namespace).await;
            
            let elapsed = start_time.elapsed().as_secs();
            ctx.trace_info(format!("Image '{}' created successfully in {} seconds", input.name, elapsed));
            
            Ok(CreateImageOutput {
                image_name: input.name,
                image_id: image_id.to_string(),
                blob_path,
                backup_size_bytes: backup_size,
                creation_time_seconds: elapsed,
            })
        }
        Err(e) => {
            ctx.trace_error(format!("Backup failed: {}", e));
            
            // Update CMS to failed
            update_image_state(&ctx, &input.name, ImageState::Failed, None, None, Some(e.clone())).await;
            
            // Cleanup job
            cleanup_job(&ctx, &job_name, &namespace).await;
            
            Err(e)
        }
    }
}

#[derive(Debug, Clone)]
struct SourceInstanceInfo {
    id: Uuid,
    k8s_name: String,
    namespace: String,
    postgres_version: String,
    storage_size_gb: i32,
    image_type: String,
}

async fn get_password_from_secret(
    ctx: &OrchestrationContext,
    k8s_name: &str,
    namespace: &str,
) -> Result<String, String> {
    let input = GetInstancePasswordInput {
        k8s_name: k8s_name.to_string(),
        namespace: namespace.to_string(),
    };
    
    let output: GetInstancePasswordOutput = ctx
        .schedule_activity_with_retry_typed::<GetInstancePasswordInput, GetInstancePasswordOutput>(
            activities::get_instance_password::NAME,
            &input,
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(2),
                    multiplier: 2.0,
                    max: Duration::from_secs(30),
                }),
        )
        .await?;
    
    if !output.found {
        return Err(format!(
            "Password secret not found for instance '{}'. Secret '{}' may not exist.",
            k8s_name, format!("{}-secret", k8s_name)
        ));
    }
    
    output.password.ok_or_else(|| {
        format!("Password key not found in secret for instance '{}'", k8s_name)
    })
}

async fn get_source_instance(
    ctx: &OrchestrationContext,
    k8s_name: &str,
) -> Result<SourceInstanceInfo, String> {
    let input = GetInstanceByK8sNameInput {
        k8s_name: k8s_name.to_string(),
    };
    
    let output: GetInstanceByK8sNameOutput = ctx
        .schedule_activity_with_retry_typed::<GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput>(
            cms::get_instance_by_k8s_name::NAME,
            &input,
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(2),
                    multiplier: 2.0,
                    max: Duration::from_secs(30),
                }),
        )
        .await?;
    
    if !output.found {
        return Err(format!("Source instance '{}' not found", k8s_name));
    }
    
    let record = output.record.ok_or("Instance record missing")?;
    
    if record.state != "running" {
        return Err(format!(
            "Source instance '{}' is not running (state: {})",
            k8s_name, record.state
        ));
    }
    
    Ok(SourceInstanceInfo {
        id: record.id,
        k8s_name: record.k8s_name,
        namespace: record.namespace,
        postgres_version: record.postgres_version,
        storage_size_gb: record.storage_size_gb,
        image_type: record.image_type,
    })
}

async fn create_cms_record(
    ctx: &OrchestrationContext,
    input: &CreateImageInput,
    instance: &SourceInstanceInfo,
    blob_storage_url: &str,
    blob_container: &str,
    blob_path: &str,
    source_password: &str,
) -> Result<Uuid, String> {
    // Encrypt password for storage (simple encoding for now - in production use proper encryption)
    let password_encrypted = source_password.as_bytes().to_vec();
    
    let op = ImageOperation::Create {
        name: input.name.clone(),
        description: input.description.clone(),
        source_instance_id: instance.id,
        source_k8s_name: instance.k8s_name.clone(),
        source_namespace: instance.namespace.clone(),
        blob_storage_url: blob_storage_url.to_string(),
        blob_container: blob_container.to_string(),
        blob_path: blob_path.to_string(),
        storage_size_gb: instance.storage_size_gb,
        postgres_version: instance.postgres_version.clone(),
        image_type: instance.image_type.clone(),
        source_password_encrypted: password_encrypted,
        orchestration_id: input.orchestration_id.clone(),
    };
    
    let result: ImageOperationResult = ctx
        .schedule_activity_with_retry_typed::<ImageOperation, ImageOperationResult>(
            cms::image_ops::NAME,
            &op,
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Linear {
                    base: Duration::from_secs(2),
                    max: Duration::from_secs(10),
                }),
        )
        .await?;
    
    match result {
        ImageOperationResult::Created { image_id } => Ok(image_id),
        _ => Err("Unexpected CMS result for image creation".to_string()),
    }
}

async fn run_backup(
    ctx: &OrchestrationContext,
    input: &CreateImageInput,
    job_name: &str,
    namespace: &str,
    instance: &SourceInstanceInfo,
    blob_storage_account: &str,
    blob_container: &str,
    blob_path: &str,
    source_password: &str,
) -> Result<(Option<i64>, Option<String>), String> {
    // Create and start the backup job
    let job_input = RunBackupJobInput {
        job_name: job_name.to_string(),
        namespace: namespace.to_string(),
        source_instance_name: instance.k8s_name.clone(),
        source_password: source_password.to_string(),
        blob_storage_account: blob_storage_account.to_string(),
        blob_container: blob_container.to_string(),
        blob_path: blob_path.to_string(),
        image_name: input.name.clone(),
        postgres_version: instance.postgres_version.clone(),
    };
    
    let _: RunBackupJobOutput = ctx
        .schedule_activity_with_retry_typed::<RunBackupJobInput, RunBackupJobOutput>(
            activities::run_backup_job::NAME,
            &job_input,
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(2),
                    multiplier: 2.0,
                    max: Duration::from_secs(30),
                }),
        )
        .await?;
    
    // Poll for job completion
    let max_wait = Duration::from_secs(3600); // 1 hour max
    let poll_interval = Duration::from_secs(10);
    let mut elapsed = Duration::ZERO;
    
    loop {
        // Wait before checking
        ctx.schedule_timer(poll_interval).await;
        elapsed += poll_interval;
        
        let status: WaitForJobOutput = ctx
            .schedule_activity_typed::<WaitForJobInput, WaitForJobOutput>(
                activities::wait_for_job::NAME,
                &WaitForJobInput {
                    job_name: job_name.to_string(),
                    namespace: namespace.to_string(),
                },
            )
            .await?;
        
        if status.succeeded {
            ctx.trace_info("Backup job completed successfully");
            // TODO: Parse backup size and checksum from job output/logs
            return Ok((None, None));
        }
        
        if status.failed {
            let msg = status.message.unwrap_or_else(|| "Unknown error".to_string());
            return Err(format!("Backup job failed: {}", msg));
        }
        
        if elapsed >= max_wait {
            return Err("Backup job timed out after 1 hour".to_string());
        }
        
        ctx.trace_info(format!("Backup job still running... ({} seconds)", elapsed.as_secs()));
    }
}

async fn update_image_state(
    ctx: &OrchestrationContext,
    name: &str,
    state: ImageState,
    backup_size_bytes: Option<i64>,
    backup_checksum: Option<String>,
    error_message: Option<String>,
) {
    let op = ImageOperation::UpdateState {
        name: name.to_string(),
        state,
        backup_size_bytes,
        backup_checksum,
        error_message,
    };
    
    let _ = ctx
        .schedule_activity_with_retry_typed::<ImageOperation, ImageOperationResult>(
            cms::image_ops::NAME,
            &op,
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Linear {
                    base: Duration::from_secs(2),
                    max: Duration::from_secs(10),
                }),
        )
        .await;
}

async fn cleanup_job(
    ctx: &OrchestrationContext,
    job_name: &str,
    namespace: &str,
) {
    let _ = ctx
        .schedule_activity_typed::<DeleteJobInput, DeleteJobOutput>(
            activities::delete_job::NAME,
            &DeleteJobInput {
                job_name: job_name.to_string(),
                namespace: namespace.to_string(),
                delete_secret: true,
            },
        )
        .await;
}
