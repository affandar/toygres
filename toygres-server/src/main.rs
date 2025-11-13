use anyhow::Result;
use clap::{Parser, Subcommand};
use duroxide::runtime::Runtime;
use duroxide::Client;
use duroxide_pg::PostgresProvider;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use toygres_orchestrations::names::orchestrations;
use toygres_orchestrations::registry::{create_activity_registry, create_orchestration_registry};
use toygres_orchestrations::types::*;
use uuid::Uuid;

mod config;
mod api;
mod worker;

/// Toygres - PostgreSQL as a Service on AKS
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a new PostgreSQL instance
    Create {
        /// DNS name for the instance (e.g., "mydb" creates mydb-<DNS_LABEL>.<region>.cloudapp.azure.com)
        name: String,
        
        /// PostgreSQL password
        #[arg(short, long)]
        password: String,
        
        /// PostgreSQL version (default: "18")
        #[arg(long, default_value = "18")]
        version: Option<String>,
        
        /// Storage size in GB (default: 10)
        #[arg(long, default_value = "10")]
        storage: Option<i32>,
        
        /// Use ClusterIP instead of LoadBalancer (no public IP)
        #[arg(long)]
        internal: bool,
        
        /// Kubernetes namespace (default: "toygres")
        #[arg(long, default_value = "toygres")]
        namespace: Option<String>,
    },
    
    /// Delete a PostgreSQL instance
    Delete {
        /// Instance name to delete
        name: String,
        
        /// Kubernetes namespace (default: "toygres")
        #[arg(long, default_value = "toygres")]
        namespace: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    // Default: info level for toygres and duroxide
                    "toygres_server=info,\
                     toygres_activities=info,\
                     toygres_orchestrations=info,\
                     duroxide=info,\
                     duroxide_pg=warn"
                        .into()
                }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Toygres Control Plane CLI");
    
    // Initialize PostgreSQL provider for Duroxide with custom schema
    // For CLI testing, use a temporary in-memory SQLite connection
    // Format: postgresql://... but duroxide-pg also supports sqlite://...
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string());
    
    let schema_name = "toygres_duroxide";
    
    tracing::info!("Connecting to Duroxide store: {} (schema: {})", 
        if db_url.starts_with("sqlite") { "SQLite (in-memory)" } else { "PostgreSQL" },
        schema_name);
    
    let store = Arc::new(PostgresProvider::new_with_schema(&db_url, Some(schema_name)).await
        .map_err(|e| anyhow::anyhow!("Failed to initialize Duroxide store: {}", e))?);
    
    // Initialize schema (creates tables if they don't exist)
    store.initialize_schema().await
        .map_err(|e| anyhow::anyhow!("Failed to initialize Duroxide schema: {}", e))?;
    
    // Create activity and orchestration registries
    let activities = Arc::new(create_activity_registry());
    let orchestrations = create_orchestration_registry();
    
    // Start Duroxide runtime
    tracing::info!("Starting Duroxide runtime");
    let runtime = Runtime::start_with_store(
        store.clone(),
        activities,
        orchestrations,
    )
    .await;
    
    // Create Duroxide client
    let client = Client::new(store);
    
    // Execute command
    match args.command {
        Commands::Create { name, password, version, storage, internal, namespace } => {
            handle_create(client, name, password, version, storage, !internal, namespace).await?;
        }
        Commands::Delete { name, namespace } => {
            handle_delete(client, name, namespace).await?;
        }
    }
    
    // Shutdown runtime
    tracing::info!("Shutting down Duroxide runtime");
    runtime.shutdown(None).await;

    Ok(())
}

async fn handle_create(
    client: Client,
    name: String,
    password: String,
    version: Option<String>,
    storage: Option<i32>,
    use_load_balancer: bool,
    namespace: Option<String>,
) -> Result<()> {
    // Generate unique instance name with 8-character GUID suffix
    let guid = Uuid::new_v4().to_string();
    let guid_suffix = &guid[..8];
    let unique_instance_name = format!("{}-{}", name, guid_suffix);
    
    tracing::info!("Creating PostgreSQL instance: {} (K8s name: {})", name, unique_instance_name);
    
    // Get DNS label from env if available
    let dns_label = std::env::var("DNS_LABEL").ok().map(|base| {
        format!("{}-{}", name, base)
    });
    
    // Build input (use unique instance name for K8s resources)
    let input = CreateInstanceInput {
        name: unique_instance_name.clone(),
        password,
        postgres_version: version,
        storage_size_gb: storage,
        use_load_balancer: Some(use_load_balancer),
        dns_label,
        namespace,
    };
    
    let input_json = serde_json::to_string(&input)?;
    let instance_id = format!("create-{}", unique_instance_name);
    
    // Start orchestration
    tracing::info!("Starting create-instance orchestration");
    client
        .start_orchestration(&instance_id, orchestrations::CREATE_INSTANCE, input_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start orchestration: {}", e))?;
    
    // Wait for completion
    tracing::info!("Waiting for orchestration to complete...");
    let status = client
        .wait_for_orchestration(&instance_id, std::time::Duration::from_secs(600))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to wait for orchestration: {:?}", e))?;
    
    match status {
        duroxide::OrchestrationStatus::Completed { output, .. } => {
            let result: CreateInstanceOutput = serde_json::from_str(&output)?;
            
            tracing::info!("✓ PostgreSQL instance created successfully!");
            tracing::info!("  User name: {}", name);
            tracing::info!("  K8s instance: {}", result.instance_name);
            tracing::info!("  Namespace: {}", result.namespace);
            tracing::info!("  PostgreSQL: {}", result.postgres_version);
            tracing::info!("  Deployment time: {} seconds", result.deployment_time_seconds);
            tracing::info!("");
            tracing::info!("Connection Strings:");
            tracing::info!("  IP:  {}", result.ip_connection_string);
            if let Some(dns_conn) = result.dns_connection_string {
                tracing::info!("  DNS: {}", dns_conn);
            }
            if let Some(dns_name) = result.dns_name {
                tracing::info!("");
                tracing::info!("Azure DNS: {}", dns_name);
            }
        }
        duroxide::OrchestrationStatus::Failed { details, .. } => {
            tracing::error!("✗ Failed to create instance: {:?}", details);
            anyhow::bail!("Orchestration failed: {:?}", details);
        }
        other => {
            tracing::warn!("Unexpected status: {:?}", other);
            anyhow::bail!("Unexpected orchestration status");
        }
    }
    
    Ok(())
}

async fn handle_delete(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    // Note: User provides the K8s instance name (with GUID suffix)
    // In Phase 3 with metadata DB, we'll look up by user-friendly name
    tracing::info!("Deleting PostgreSQL instance: {}", name);
    
    // Build input
    let input = DeleteInstanceInput {
        name: name.clone(),
        namespace,
    };
    
    let input_json = serde_json::to_string(&input)?;
    let instance_id = format!("delete-{}", name);
    
    // Start orchestration
    tracing::info!("Starting delete-instance orchestration");
    client
        .start_orchestration(&instance_id, orchestrations::DELETE_INSTANCE, input_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start orchestration: {}", e))?;
    
    // Wait for completion
    tracing::info!("Waiting for orchestration to complete...");
    let status = client
        .wait_for_orchestration(&instance_id, std::time::Duration::from_secs(120))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to wait for orchestration: {:?}", e))?;
    
    match status {
        duroxide::OrchestrationStatus::Completed { output, .. } => {
            let result: DeleteInstanceOutput = serde_json::from_str(&output)?;
            
            if result.deleted {
                tracing::info!("✓ PostgreSQL instance deleted successfully!");
            } else {
                tracing::info!("Instance '{}' did not exist", result.instance_name);
            }
            tracing::info!("  Instance: {}", result.instance_name);
        }
        duroxide::OrchestrationStatus::Failed { details, .. } => {
            tracing::error!("✗ Failed to delete instance: {:?}", details);
            anyhow::bail!("Orchestration failed: {:?}", details);
        }
        other => {
            tracing::warn!("Unexpected status: {:?}", other);
            anyhow::bail!("Unexpected orchestration status");
        }
    }
    
    Ok(())
}

