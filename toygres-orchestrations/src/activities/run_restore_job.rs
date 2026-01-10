//! Activity to run a Kubernetes Job that restores a PostgreSQL backup from Azure Blob Storage.
//!
//! This activity creates a K8s Job that:
//! 1. Downloads the backup from Azure Blob Storage using azcopy with managed identity
//! 2. Verifies the checksum
//! 3. Extracts the backup to the target PVC
//! 4. Sets proper PostgreSQL permissions

use k8s_openapi::api::batch::v1::Job;
use kube::{api::PostParams, Api, Client};
use serde::{Deserialize, Serialize};
use tera::{Context as TeraContext, Tera};

pub const NAME: &str = "toygres-orchestrations::activity::run-restore-job";

/// Input for the run_restore_job activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRestoreJobInput {
    /// Name of the K8s Job to create
    pub job_name: String,
    /// Namespace for the Job
    pub namespace: String,
    /// Name of the instance being restored to
    pub instance_name: String,
    /// PVC to restore to
    pub pvc_name: String,
    /// PostgreSQL version (for container image)
    pub postgres_version: String,
    /// Azure Blob Storage account name
    pub blob_storage_account: String,
    /// Azure Blob Storage container name
    pub blob_container: String,
    /// Path within the container (e.g., "instance-name/2024-01-15T10:30:00Z")
    pub blob_path: String,
}

/// Output for the run_restore_job activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRestoreJobOutput {
    /// Name of the created Job
    pub job_name: String,
}

pub async fn activity(
    _ctx: duroxide::ActivityContext,
    input: RunRestoreJobInput,
) -> Result<RunRestoreJobOutput, String> {
    tracing::info!(
        job_name = %input.job_name,
        instance_name = %input.instance_name,
        pvc_name = %input.pvc_name,
        blob_path = %input.blob_path,
        "Running restore job"
    );

    // Load and render the restore job template
    let template_str = include_str!("../templates/restore-job.yaml");
    let mut tera = Tera::default();
    tera.add_raw_template("restore-job", template_str)
        .map_err(|e| format!("Failed to parse restore job template: {}", e))?;

    let mut context = TeraContext::new();
    context.insert("job_name", &input.job_name);
    context.insert("namespace", &input.namespace);
    context.insert("instance_name", &input.instance_name);
    context.insert("pvc_name", &input.pvc_name);
    context.insert("postgres_version", &input.postgres_version);
    context.insert("blob_storage_account", &input.blob_storage_account);
    context.insert("blob_container", &input.blob_container);
    context.insert("blob_path", &input.blob_path);

    let rendered = tera
        .render("restore-job", &context)
        .map_err(|e| format!("Failed to render restore job template: {}", e))?;

    tracing::debug!(rendered_yaml = %rendered, "Restore job YAML");

    // Parse and create the Job
    let job: Job = serde_yaml::from_str(&rendered)
        .map_err(|e| format!("Failed to parse restore job YAML: {}", e))?;

    let client = Client::try_default()
        .await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;

    let jobs_api: Api<Job> = Api::namespaced(client, &input.namespace);

    jobs_api
        .create(&PostParams::default(), &job)
        .await
        .map_err(|e| format!("Failed to create restore job: {}", e))?;

    tracing::info!(job_name = %input.job_name, "Restore job created");

    Ok(RunRestoreJobOutput {
        job_name: input.job_name,
    })
}
