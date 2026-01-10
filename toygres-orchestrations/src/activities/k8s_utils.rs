//! Kubernetes utility activities
//!
//! Consolidated module for small K8s utility activities:
//! - create_pvc: Create PersistentVolumeClaim
//! - delete_job: Delete a K8s Job
//! - wait_for_job: Check K8s Job status
//! - get_instance_password: Read password from K8s Secret

use std::future::Future;
use std::time::Duration;

use duroxide::ActivityContext;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Secret};
use kube::api::{Api, DeleteParams, PostParams};
use kube::Client;
use tera::{Context as TeraContext, Tera};

use crate::activity_types::{
    CreatePvcInput, CreatePvcOutput,
    DeleteJobInput, DeleteJobOutput,
    GetInstancePasswordInput, GetInstancePasswordOutput,
    WaitForJobInput, WaitForJobOutput,
};
use crate::k8s_client::get_k8s_client;

// ============================================================================
// Retry Helper for K8s Operations
// ============================================================================

/// Configuration for K8s operation retries
struct RetryConfig {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
        }
    }
}

/// Determines if a K8s error is retryable (transient network/API issues)
fn is_retryable_error(err: &kube::Error) -> bool {
    match err {
        // Transient API errors (5xx, rate limiting)
        kube::Error::Api(response) => {
            response.code == 429 || response.code >= 500
        }
        // Network/connection errors - check error message for common transient patterns
        kube::Error::Service(e) => {
            let msg = e.to_string().to_lowercase();
            msg.contains("connection") || msg.contains("timeout") || msg.contains("reset")
        }
        kube::Error::HyperError(e) => {
            let msg = e.to_string().to_lowercase();
            msg.contains("connection") || msg.contains("timeout") || msg.contains("reset")
        }
        // Not retryable: 4xx errors (except 429), serialization errors, etc.
        _ => false,
    }
}

/// Executes a K8s operation with exponential backoff retry for transient errors
async fn with_k8s_retry<T, F, Fut>(
    operation_name: &str,
    config: RetryConfig,
    mut operation: F,
) -> Result<T, kube::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, kube::Error>>,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Only retry transient errors
                if !is_retryable_error(&e) || attempt >= config.max_attempts {
                    if attempt >= config.max_attempts {
                        tracing::warn!(
                            operation = operation_name,
                            attempt = attempt,
                            error = %e,
                            "K8s operation failed after max retries"
                        );
                    }
                    return Err(e);
                }

                tracing::debug!(
                    operation = operation_name,
                    attempt = attempt,
                    delay_ms = delay.as_millis() as u64,
                    error = %e,
                    "K8s operation failed with transient error, retrying"
                );

                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(config.max_delay);
            }
        }
    }
}

// ============================================================================
// Activity Names
// ============================================================================

pub mod names {
    pub const CREATE_PVC: &str = "toygres-orchestrations::activity::create-pvc";
    pub const DELETE_JOB: &str = "toygres-orchestrations::activity::delete-job";
    pub const WAIT_FOR_JOB: &str = "toygres-orchestrations::activity::wait-for-job";
    pub const GET_INSTANCE_PASSWORD: &str = "toygres-orchestrations::activity::get-instance-password";
}

// ============================================================================
// Create PVC Activity
// ============================================================================

/// Creates a PersistentVolumeClaim for PostgreSQL data storage.
/// Used during restore operations to create the PVC before the restore job runs.
/// For normal deployments, the PVC is created as part of deploy_postgres.
pub async fn create_pvc(
    ctx: ActivityContext,
    input: CreatePvcInput,
) -> Result<CreatePvcOutput, String> {
    // The template adds "-pvc" suffix to the name
    let actual_pvc_name = format!("{}-pvc", input.name);
    ctx.trace_info(format!("Creating PVC: {}/{}", input.namespace, actual_pvc_name));

    let client = Client::try_default()
        .await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client, &input.namespace);

    // Check if PVC already exists (idempotency)
    match pvcs.get(&actual_pvc_name).await {
        Ok(_) => {
            ctx.trace_info("PVC already exists, skipping creation");
            return Ok(CreatePvcOutput {
                pvc_name: actual_pvc_name,
                created: false,
            });
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            // PVC doesn't exist, proceed with creation
        }
        Err(e) => {
            return Err(format!("Failed to check if PVC exists: {}", e));
        }
    }

    // Load and render the PVC template
    let template_str = include_str!("../templates/postgres-pvc.yaml");
    let mut tera = Tera::default();
    tera.add_raw_template("pvc", template_str)
        .map_err(|e| format!("Failed to parse PVC template: {}", e))?;

    let mut template_ctx = TeraContext::new();
    template_ctx.insert("name", &input.name);
    template_ctx.insert("namespace", &input.namespace);
    template_ctx.insert("storage_size", &input.storage_size_gb);

    let rendered = tera
        .render("pvc", &template_ctx)
        .map_err(|e| format!("Failed to render PVC template: {}", e))?;

    let pvc: PersistentVolumeClaim = serde_yaml::from_str(&rendered)
        .map_err(|e| format!("Failed to parse PVC YAML: {}", e))?;

    pvcs.create(&PostParams::default(), &pvc)
        .await
        .map_err(|e| format!("Failed to create PVC: {}", e))?;

    ctx.trace_info("PVC created successfully");

    Ok(CreatePvcOutput {
        pvc_name: actual_pvc_name,
        created: true,
    })
}

// ============================================================================
// Delete Job Activity
// ============================================================================

/// Deletes a K8s Job and its pods.
/// Includes automatic retry for transient K8s API failures.
pub async fn delete_job(
    ctx: ActivityContext,
    input: DeleteJobInput,
) -> Result<DeleteJobOutput, String> {
    ctx.trace_info(format!("Deleting job: {}", input.job_name));

    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

    let jobs: Api<Job> = Api::namespaced(client.clone(), &input.namespace);

    // Delete the job with propagation policy to clean up pods
    let dp = DeleteParams {
        propagation_policy: Some(kube::api::PropagationPolicy::Background),
        ..Default::default()
    };

    let job_name = input.job_name.clone();
    let result = with_k8s_retry(
        "delete_job",
        RetryConfig::default(),
        || {
            let jobs = jobs.clone();
            let dp = dp.clone();
            let job_name = job_name.clone();
            async move {
                jobs.delete(&job_name, &dp).await
            }
        },
    ).await;

    match result {
        Ok(_) => {
            ctx.trace_info(format!("Job {} deleted", input.job_name));
        }
        Err(kube::Error::Api(response)) if response.code == 404 => {
            ctx.trace_info(format!("Job {} already deleted", input.job_name));
        }
        Err(e) => {
            return Err(format!("Failed to delete job: {}", e));
        }
    }

    // Note: We no longer create job-specific secrets - backup jobs use the instance's existing secret
    // So no secret cleanup is needed here

    Ok(DeleteJobOutput { deleted: true })
}

// ============================================================================
// Wait For Job Activity
// ============================================================================

/// Checks the status of a K8s Job. Returns current state so the orchestration
/// can poll with timers if still in progress.
/// Includes automatic retry for transient K8s API failures.
pub async fn wait_for_job(
    ctx: ActivityContext,
    input: WaitForJobInput,
) -> Result<WaitForJobOutput, String> {
    ctx.trace_info(format!("Checking job status: {}", input.job_name));

    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

    let jobs: Api<Job> = Api::namespaced(client.clone(), &input.namespace);

    let job_name = input.job_name.clone();
    let job = with_k8s_retry(
        "wait_for_job::get",
        RetryConfig::default(),
        || {
            let jobs = jobs.clone();
            let job_name = job_name.clone();
            async move {
                jobs.get(&job_name).await
            }
        },
    ).await.map_err(|e| format!("Failed to get job: {}", e))?;

    let status = job.status.as_ref();

    let succeeded = status.and_then(|s| s.succeeded).unwrap_or(0) > 0;
    let failed = status.and_then(|s| s.failed).unwrap_or(0) > 0;
    let active = status.and_then(|s| s.active).unwrap_or(0) > 0;

    let completion_time = status
        .and_then(|s| s.completion_time.as_ref())
        .map(|t| t.0.to_rfc3339());

    // Get failure message if failed
    let message = if failed {
        get_job_failure_message(&job)
    } else {
        None
    };

    ctx.trace_info(format!(
        "Job status - succeeded: {}, failed: {}, active: {}",
        succeeded, failed, active
    ));

    Ok(WaitForJobOutput {
        succeeded,
        failed,
        active,
        completion_time,
        message,
    })
}

fn get_job_failure_message(job: &Job) -> Option<String> {
    let status = job.status.as_ref()?;

    // Check conditions for failure reason
    if let Some(conditions) = &status.conditions {
        for condition in conditions {
            if condition.type_ == "Failed" && condition.status == "True" {
                return condition.message.clone();
            }
        }
    }

    // Generic failure message
    if status.failed.unwrap_or(0) > 0 {
        return Some("Job failed - check pod logs for details".to_string());
    }

    None
}

// ============================================================================
// Get Instance Password Activity
// ============================================================================

/// Reads the PostgreSQL password from the K8s Secret associated with an instance.
/// The secret is named "{instance_name}-secret".
/// Includes automatic retry for transient K8s API failures.
pub async fn get_instance_password(
    _ctx: ActivityContext,
    input: GetInstancePasswordInput,
) -> Result<GetInstancePasswordOutput, String> {
    tracing::info!(
        k8s_name = %input.k8s_name,
        namespace = %input.namespace,
        "Getting instance password from K8s Secret"
    );

    let client = Client::try_default()
        .await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

    let secrets: Api<Secret> = Api::namespaced(client, &input.namespace);

    // The secret is named "{instance_name}-secret"
    let secret_name = format!("{}-secret", input.k8s_name);

    let secret_name_clone = secret_name.clone();
    let result = with_k8s_retry(
        "get_instance_password::get",
        RetryConfig::default(),
        || {
            let secrets = secrets.clone();
            let secret_name = secret_name_clone.clone();
            async move {
                secrets.get(&secret_name).await
            }
        },
    ).await;

    match result {
        Ok(secret) => {
            let password = secret
                .data
                .as_ref()
                .and_then(|data| data.get("POSTGRES_PASSWORD"))
                .and_then(|bytes| String::from_utf8(bytes.0.clone()).ok());

            if let Some(ref pwd) = password {
                tracing::info!(
                    secret_name = %secret_name,
                    "Password retrieved from K8s Secret"
                );
                Ok(GetInstancePasswordOutput {
                    found: true,
                    password: Some(pwd.clone()),
                })
            } else {
                tracing::warn!(
                    secret_name = %secret_name,
                    "Secret found but POSTGRES_PASSWORD key missing"
                );
                Ok(GetInstancePasswordOutput {
                    found: false,
                    password: None,
                })
            }
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            tracing::warn!(
                secret_name = %secret_name,
                "Secret not found"
            );
            Ok(GetInstancePasswordOutput {
                found: false,
                password: None,
            })
        }
        Err(e) => Err(format!("Failed to get secret: {}", e)),
    }
}
