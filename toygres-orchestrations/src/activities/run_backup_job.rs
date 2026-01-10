//! Run backup job activity
//! 
//! Creates a K8s Job that runs pg_basebackup and uploads to Azure Blob Storage.

use duroxide::ActivityContext;
use k8s_openapi::api::batch::v1::Job;
use kube::api::{Api, PostParams};
use tera::{Tera, Context as TeraContext};

use crate::activity_types::{RunBackupJobInput, RunBackupJobOutput};
use crate::k8s_client::get_k8s_client;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::run-backup-job";

pub async fn activity(
    ctx: ActivityContext,
    input: RunBackupJobInput,
) -> Result<RunBackupJobOutput, String> {
    ctx.trace_info(format!(
        "Creating backup job: {} for instance {}",
        input.job_name, input.source_instance_name
    ));
    
    let client = get_k8s_client().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // Check if job already exists (idempotency)
    let jobs: Api<Job> = Api::namespaced(client.clone(), &input.namespace);
    match jobs.get(&input.job_name).await {
        Ok(_) => {
            ctx.trace_info("Backup job already exists (replay), skipping creation");
            return Ok(RunBackupJobOutput {
                job_name: input.job_name,
                created: false,
            });
        }
        Err(kube::Error::Api(response)) if response.code == 404 => {
            // Job doesn't exist, we'll create it
        }
        Err(e) => {
            return Err(format!("Failed to check if job exists: {}", e));
        }
    }
    
    // Use the instance's existing secret (format: {instance-name}-secret with key POSTGRES_PASSWORD)
    let secret_name = format!("{}-secret", input.source_instance_name);
    
    // Create the backup job
    create_backup_job(&client, &input, &secret_name).await?;
    
    ctx.trace_info(format!("Backup job created: {}", input.job_name));
    
    Ok(RunBackupJobOutput {
        job_name: input.job_name,
        created: true,
    })
}

async fn create_backup_job(
    client: &kube::Client,
    input: &RunBackupJobInput,
    secret_name: &str,
) -> Result<(), String> {
    let mut tera = Tera::default();
    let template = include_str!("../templates/backup-job.yaml");
    tera.add_raw_template("backup-job.yaml", template)
        .map_err(|e| format!("Failed to load backup job template: {}", e))?;
    
    let mut context = TeraContext::new();
    context.insert("job_name", &input.job_name);
    context.insert("namespace", &input.namespace);
    context.insert("source_instance_name", &input.source_instance_name);
    context.insert("secret_name", secret_name);
    context.insert("blob_storage_account", &input.blob_storage_account);
    context.insert("blob_container", &input.blob_container);
    context.insert("blob_path", &input.blob_path);
    context.insert("image_name", &input.image_name);
    context.insert("postgres_version", &input.postgres_version);
    
    let yaml = tera.render("backup-job.yaml", &context)
        .map_err(|e| format!("Failed to render backup job template: {}", e))?;
    
    let job: Job = serde_yaml::from_str(&yaml)
        .map_err(|e| format!("Failed to parse backup job YAML: {}", e))?;
    
    let jobs: Api<Job> = Api::namespaced(client.clone(), &input.namespace);
    jobs.create(&PostParams::default(), &job).await
        .map_err(|e| format!("Failed to create backup job: {}", e))?;
    
    Ok(())
}
