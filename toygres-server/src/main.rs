use anyhow::Result;
use clap::{Parser, Subcommand};
use duroxide::runtime::{Runtime, RuntimeOptions};
use duroxide::Client;
use duroxide_pg::PostgresProvider;
use reqwest::StatusCode;
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
    mode: Mode,
}

#[derive(Subcommand, Debug)]
enum Mode {
    /// Run as standalone server (API + Workers)
    Standalone {
        /// API port
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Number of worker threads
        #[arg(short, long, default_value = "1")]
        workers: usize,
    },
    
    /// Run as API server only (no workers)
    Api {
        /// API port
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    
    /// Run as worker only (no API)
    Worker {
        /// Worker ID
        #[arg(long)]
        worker_id: Option<String>,
    },
    
    /// Create a new PostgreSQL instance
    Create {
        /// DNS name for the instance (e.g., "mydb" creates mydb.<region>.cloudapp.azure.com)
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
        /// DNS name of the instance to delete (e.g., "adardb5")
        name: String,
        
        /// Kubernetes namespace (default: "toygres")
        #[arg(long, default_value = "toygres")]
        namespace: Option<String>,
    },
    
    /// List all PostgreSQL instances
    List {
        /// Output format
        #[arg(short, long, default_value = "table")]
        output: String,
    },
    
    /// Get details of a specific instance
    Get {
        /// DNS name of the instance
        name: String,
        
        /// Output format
        #[arg(short, long, default_value = "table")]
        output: String,
    },
    
    /// Manage local development server
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ServerCommand {
    /// Start the server in background
    Start {
        /// API port
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Run in foreground (default: background)
        #[arg(short, long)]
        foreground: bool,
    },
    
    /// Stop the server
    Stop,
    
    /// Check if server is running
    Status,
    
    /// View server logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "100")]
        tail: usize,
    },
    
    /// List orchestrations (advanced diagnostics)
    Orchestrations {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        
        /// Limit number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    
    /// Get orchestration details (advanced diagnostics)
    Orchestration {
        /// Orchestration ID
        id: String,
        
        /// Show execution history
        #[arg(long)]
        history: bool,
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

    // Route to appropriate mode
    match args.mode {
        Mode::Standalone { port, workers } => {
            run_standalone_mode(port, workers).await
        }
        Mode::Api { port } => {
            run_api_mode(port).await
        }
        Mode::Worker { worker_id } => {
            run_worker_mode(worker_id).await
        }
        Mode::Create { name, password, version, storage, internal, namespace } => {
            run_create_command(name, password, version, storage, internal, namespace).await
        }
        Mode::Delete { name, namespace } => {
            run_delete_command(name, namespace).await
        }
        Mode::List { output } => {
            run_list_command(output).await
        }
        Mode::Get { name, output } => {
            run_get_command(name, output).await
        }
        Mode::Server { command } => {
            run_server_command(command).await
        }
    }
}

async fn run_standalone_mode(port: u16, _workers: usize) -> Result<()> {
    tracing::info!("Starting Toygres in standalone mode (API + Workers)");
    tracing::info!("API port: {}", port);
    
    // Initialize Duroxide
    let (runtime, store) = initialize_duroxide().await?;
    
    // Create API state
    let client = Arc::new(Client::new(store.clone()));
    let state = api::AppState {
        duroxide_client: client,
        store: store.clone(),
    };
    
    // Start API server
    tracing::info!("Starting API server on 0.0.0.0:{}", port);
    
    // Spawn API server task
    let api_handle = tokio::spawn(async move {
        if let Err(e) = api::start_server(port, state).await {
            tracing::error!("API server error: {}", e);
        }
    });
    
    tracing::info!("✓ Toygres server ready");
    tracing::info!("  API: http://0.0.0.0:{}", port);
    tracing::info!("  Press Ctrl+C to stop");
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    
    tracing::info!("Shutting down...");
    api_handle.abort();
    
    tracing::info!("Shutting down Duroxide runtime");
    runtime.shutdown(None).await;
    
    Ok(())
}

async fn run_api_mode(port: u16) -> Result<()> {
    tracing::info!("Starting Toygres in API-only mode");
    tracing::info!("API port: {}", port);
    
    // TODO: Implement API mode
    // - Start API server
    // - No workers (just Duroxide client)
    
    anyhow::bail!("API mode not yet implemented")
}

async fn run_worker_mode(worker_id: Option<String>) -> Result<()> {
    let id = worker_id.unwrap_or_else(|| format!("worker-{}", Uuid::new_v4()));
    tracing::info!("Starting Toygres in worker-only mode");
    tracing::info!("Worker ID: {}", id);
    
    // TODO: Implement worker mode
    // - Start Duroxide runtime with workers
    // - No API server
    
    anyhow::bail!("Worker mode not yet implemented")
}

async fn run_list_command(output: String) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/instances", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let instances: Vec<serde_json::Value> = response.json().await?;
    
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(&instances)?);
    } else {
        // Table format
        println!("{:<15} {:<20} {:<10} {:<10} {:<8} {:<10}", 
                 "NAME", "DNS NAME", "STATE", "HEALTH", "VERSION", "STORAGE");
        println!("{}", "-".repeat(85));
        
        for inst in &instances {
            let name = inst["user_name"].as_str().unwrap_or("-");
            let dns = inst["dns_name"].as_str().unwrap_or("-");
            let state = inst["state"].as_str().unwrap_or("-");
            let health = inst["health_status"].as_str().unwrap_or("-");
            let version = inst["postgres_version"].as_str().unwrap_or("-");
            let storage = inst["storage_size_gb"].as_i64().unwrap_or(0);
            
            println!("{:<15} {:<20} {:<10} {:<10} {:<8} {}GB", 
                     name, dns, state, health, version, storage);
        }
        
        println!();
        println!("{} instance(s) found", instances.len());
    }
    
    Ok(())
}

async fn run_get_command(name: String, output: String) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/instances/{}", api_url, name))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if response.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("Instance '{}' not found", name);
    }
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let instance: serde_json::Value = response.json().await?;
    
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(&instance)?);
    } else {
        // Table format
        println!("Instance: {}", name);
        println!("{}", "=".repeat(60));
        println!();
        println!("Status:");
        println!("  State:              {}", instance["state"].as_str().unwrap_or("-"));
        println!("  Health:             {}", instance["health_status"].as_str().unwrap_or("-"));
        println!("  PostgreSQL Version: {}", instance["postgres_version"].as_str().unwrap_or("-"));
        println!();
        println!("Identity:");
        println!("  User Name:          {}", instance["user_name"].as_str().unwrap_or("-"));
        println!("  K8s Name:           {}", instance["k8s_name"].as_str().unwrap_or("-"));
        println!("  DNS Name:           {}", instance["dns_name"].as_str().unwrap_or("-"));
        println!();
        println!("Configuration:");
        println!("  Storage:            {} GB", instance["storage_size_gb"].as_i64().unwrap_or(0));
        println!("  Load Balancer:      {}", instance["use_load_balancer"].as_bool().unwrap_or(false));
        println!();
        println!("Network:");
        if let Some(dns_conn) = instance["dns_connection_string"].as_str() {
            println!("  DNS Connection:     {}", dns_conn);
        }
        if let Some(ip_conn) = instance["ip_connection_string"].as_str() {
            println!("  IP Connection:      {}", ip_conn);
        }
        if let Some(external_ip) = instance["external_ip"].as_str() {
            println!("  External IP:        {}", external_ip);
        }
        println!();
        println!("Timestamps:");
        println!("  Created:            {}", instance["created_at"].as_str().unwrap_or("-"));
        println!("  Updated:            {}", instance["updated_at"].as_str().unwrap_or("-"));
    }
    
    Ok(())
}

async fn run_server_command(command: ServerCommand) -> Result<()> {
    use std::path::PathBuf;
    
    // Get paths for PID and log files
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let toygres_dir = PathBuf::from(home).join(".toygres");
    let pid_file = toygres_dir.join("server.pid");
    let log_file = toygres_dir.join("server.log");
    
    match command {
        ServerCommand::Start { port, foreground } => {
            server_start(port, foreground, &pid_file, &log_file).await
        }
        ServerCommand::Stop => {
            server_stop(&pid_file).await
        }
        ServerCommand::Status => {
            server_status(&pid_file).await
        }
        ServerCommand::Logs { follow, tail } => {
            server_logs(&log_file, follow, tail).await
        }
        ServerCommand::Orchestrations { status, limit } => {
            server_orchestrations(status, limit).await
        }
        ServerCommand::Orchestration { id, history } => {
            server_orchestration(&id, history).await
        }
    }
}

async fn run_create_command(
    name: String,
    password: String,
    version: Option<String>,
    storage: Option<i32>,
    internal: bool,
    namespace: Option<String>,
) -> Result<()> {
    tracing::info!("Toygres Control Plane CLI");
    
    // Initialize Duroxide
    let (runtime, store) = initialize_duroxide().await?;
    
    // Create Duroxide client
    let client = Client::new(store);
    
    // Execute create command
    handle_create(client, name, password, version, storage, !internal, namespace).await?;
    
    // Shutdown runtime
    tracing::info!("Shutting down Duroxide runtime");
    runtime.shutdown(None).await;

    Ok(())
}

async fn run_delete_command(
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    tracing::info!("Toygres Control Plane CLI");
    
    // Initialize Duroxide
    let (runtime, store) = initialize_duroxide().await?;
    
    // Create Duroxide client
    let client = Client::new(store);
    
    // Execute delete command
    handle_delete(client, name, namespace).await?;
    
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
    
    // Use the user-provided name directly as the DNS label
    // This creates DNS names like: <name>.<region>.cloudapp.azure.com
    let dns_label = Some(name.clone());
    
    let instance_id = format!("create-{}", unique_instance_name);
    
    // Build input (use unique instance name for K8s resources)
    let input = CreateInstanceInput {
        user_name: name.clone(),
        name: unique_instance_name.clone(),
        password,
        postgres_version: version,
        storage_size_gb: storage,
        use_load_balancer: Some(use_load_balancer),
        dns_label,
        namespace,
        orchestration_id: instance_id.clone(),
    };
    
    let input_json = serde_json::to_string(&input)?;
    
    // Start orchestration (non-blocking)
    client
        .start_orchestration(&instance_id, orchestrations::CREATE_INSTANCE, input_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start orchestration: {}", e))?;
    
    // Return immediately - user can check status with 'get' command
    println!("✓ Instance creation started");
    println!();
    println!("  Name:           {}", name);
    println!("  K8s Name:       {}", unique_instance_name);
    println!("  DNS (expected): {}.westus3.cloudapp.azure.com", name);
    println!();
    println!("The instance is being created in the background.");
    println!();
    println!("Check status with:");
    println!("  ./toygres get {}", name);
    println!();
    println!("For advanced diagnostics:");
    println!("  ./toygres server orchestration {}", instance_id);
    
    Ok(())
}

async fn handle_delete(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    tracing::info!("Deleting PostgreSQL instance: {}", name);
    
    // Look up the K8s name by user_name in the CMS database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string());
    
    let k8s_name = if !db_url.starts_with("sqlite") {
        lookup_k8s_name_by_user_name(&db_url, &name).await?
    } else {
        // For SQLite testing, assume name is the k8s_name
        name.clone()
    };
    
    tracing::info!("Resolved to K8s instance: {}", k8s_name);
    
    let instance_id = format!("delete-{}", k8s_name);
    
    // Build input (use k8s_name for deletion)
    let input = DeleteInstanceInput {
        name: k8s_name.clone(),
        namespace,
        orchestration_id: instance_id.clone(),
    };
    
    let input_json = serde_json::to_string(&input)?;
    
    // Start orchestration (non-blocking)
    client
        .start_orchestration(&instance_id, orchestrations::DELETE_INSTANCE, input_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start orchestration: {}", e))?;
    
    // Return immediately - user can check status with 'get' command
    println!("✓ Instance deletion started");
    println!();
    println!("  Name:     {}", name);
    println!("  K8s Name: {}", k8s_name);
    println!();
    println!("The instance is being deleted in the background.");
    println!();
    println!("Check status with:");
    println!("  ./toygres get {}", name);
    println!();
    println!("For advanced diagnostics:");
    println!("  ./toygres server orchestration {}", instance_id);
    
    Ok(())
}

async fn initialize_cms_schema(db_url: &str) -> Result<()> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .context("Failed to connect to database for CMS schema initialization")?;
    
    // Create schema if it doesn't exist
    sqlx::query("CREATE SCHEMA IF NOT EXISTS toygres_cms")
        .execute(&pool)
        .await
        .context("Failed to create toygres_cms schema")?;
    
    tracing::info!("✓ CMS schema ready");
    
    Ok(())
}

async fn verify_cms_tables(db_url: &str) -> Result<()> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .context("Failed to connect to database for CMS table verification")?;
    
    // Check if the instances table exists
    let result: Option<(bool,)> = sqlx::query_as(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'toygres_cms' 
            AND table_name = 'instances'
        )"
    )
    .fetch_optional(&pool)
    .await
    .context("Failed to check if CMS tables exist")?;
    
    match result {
        Some((true,)) => {
            tracing::info!("✓ CMS tables verified");
            Ok(())
        }
        _ => {
            anyhow::bail!(
                "CMS tables not found. Please run: ./scripts/db-init.sh\n\
                 This will create the required tables in the toygres_cms schema."
            )
        }
    }
}

async fn lookup_k8s_name_by_user_name(db_url: &str, dns_name: &str) -> Result<String> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .context("Failed to connect to database for instance lookup")?;
    
    // Look up the k8s_name by dns_name
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT k8s_name FROM toygres_cms.instances 
         WHERE dns_name = $1 
         AND state != 'deleted'
         ORDER BY created_at DESC 
         LIMIT 1"
    )
    .bind(dns_name)
    .fetch_optional(&pool)
    .await
    .context("Failed to look up instance by DNS name")?;
    
    match result {
        Some((k8s_name,)) => Ok(k8s_name),
        None => anyhow::bail!(
            "Instance with DNS name '{}' not found in database. \n\
             Note: Use the DNS name you provided during creation (e.g., 'adardb5'), not the K8s name with GUID suffix.",
            dns_name
        ),
    }
}

// ============================================================================
// Server Management Functions
// ============================================================================

async fn server_start(
    port: u16,
    foreground: bool,
    pid_file: &std::path::Path,
    log_file: &std::path::Path,
) -> Result<()> {
    use std::process::{Command, Stdio};
    
    // Check if server is already running
    if is_server_running(pid_file)? {
        let pid = read_pid(pid_file)?;
        tracing::warn!("Server is already running (PID: {})", pid);
        println!("✓ Server is already running");
        println!("  PID: {}", pid);
        println!("  Use 'toygres-server server status' for details");
        return Ok(());
    }
    
    if foreground {
        // Run in foreground (blocks)
        println!("Starting Toygres server in foreground mode...");
        println!("Press Ctrl+C to stop");
        return run_standalone_mode(port, 1).await;
    }
    
    // Ensure .toygres directory exists
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Get path to current executable
    let exe = std::env::current_exe()?;
    
    println!("Starting Toygres server in background...");
    
    // Start server as background process
    let log_file_handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;
    
    let child = Command::new(exe)
        .args(["standalone", "--port", &port.to_string()])
        .stdout(Stdio::from(log_file_handle.try_clone()?))
        .stderr(Stdio::from(log_file_handle))
        .spawn()?;
    
    // Save PID
    std::fs::write(pid_file, child.id().to_string())?;
    
    // Wait a moment for server to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Check if it's still running
    if is_server_running(pid_file)? {
        println!("✓ Server started successfully");
        println!("  PID: {}", child.id());
        println!("  API: http://localhost:{}", port);
        println!("  Logs: {}", log_file.display());
        println!();
        println!("Use './toygres server stop' to stop the server");
        println!("Use './toygres server logs -f' to view logs");
        Ok(())
    } else {
        std::fs::remove_file(pid_file).ok();
        anyhow::bail!("Server failed to start. Check logs at: {}", log_file.display())
    }
}

async fn server_stop(pid_file: &std::path::Path) -> Result<()> {
    if !pid_file.exists() {
        println!("✗ Server is not running (no PID file found)");
        return Ok(());
    }
    
    let pid = read_pid(pid_file)?;
    
    println!("Stopping Toygres server (PID: {})...", pid);
    
    // Send SIGTERM for graceful shutdown
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        
        if let Err(e) = kill(Pid::from_raw(pid), Signal::SIGTERM) {
            println!("✗ Failed to send stop signal: {}", e);
            std::fs::remove_file(pid_file).ok();
            return Err(anyhow::anyhow!("Process may not be running"));
        }
    }
    
    #[cfg(not(unix))]
    {
        println!("⚠️  Graceful shutdown not supported on this platform");
        return Err(anyhow::anyhow!("Platform not supported for server management"));
    }
    
    // Wait for process to stop (up to 30 seconds)
    for i in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        if !is_server_running(pid_file)? {
            std::fs::remove_file(pid_file).ok();
            println!("✓ Server stopped successfully");
            return Ok(());
        }
        
        if i == 29 {
            println!("⚠️  Server did not stop gracefully, force killing...");
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                kill(Pid::from_raw(pid), Signal::SIGKILL).ok();
            }
            std::fs::remove_file(pid_file).ok();
        }
    }
    
    Ok(())
}

async fn server_status(pid_file: &std::path::Path) -> Result<()> {
    if !pid_file.exists() {
        println!("✗ Server is not running");
        return Ok(());
    }
    
    let pid = read_pid(pid_file)?;
    
    if is_server_running(pid_file)? {
        println!("✓ Server is running");
        println!("  PID: {}", pid);
        println!("  API: http://localhost:8080");
        
        // Try to get health info
        if let Ok(response) = reqwest::get("http://localhost:8080/health").await {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                println!("  Status: {}", json.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"));
                println!("  Version: {}", json.get("version").and_then(|v| v.as_str()).unwrap_or("unknown"));
            }
        }
    } else {
        println!("✗ Server is not running (stale PID file)");
        std::fs::remove_file(pid_file).ok();
    }
    
    Ok(())
}

async fn server_logs(log_file: &std::path::Path, follow: bool, tail: usize) -> Result<()> {
    if !log_file.exists() {
        println!("✗ No log file found at: {}", log_file.display());
        println!("  Server may not have been started yet");
        return Ok(());
    }
    
    if follow {
        // Follow logs (like tail -f)
        println!("Following logs from: {}", log_file.display());
        println!("Press Ctrl+C to stop");
        println!();
        
        // Use tail command on Unix
        #[cfg(unix)]
        {
            let status = std::process::Command::new("tail")
                .args(["-f", "-n", &tail.to_string(), log_file.to_str().unwrap()])
                .status()?;
            
            if !status.success() {
                anyhow::bail!("Failed to tail logs");
            }
        }
        
        #[cfg(not(unix))]
        {
            anyhow::bail!("Follow mode not supported on this platform");
        }
    } else {
        // Show last N lines
        use std::io::{BufRead, BufReader};
        
        let file = std::fs::File::open(log_file)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
        
        let start = if lines.len() > tail { lines.len() - tail } else { 0 };
        
        for line in &lines[start..] {
            println!("{}", line);
        }
    }
    
    Ok(())
}

fn is_server_running(pid_file: &std::path::Path) -> Result<bool> {
    if !pid_file.exists() {
        return Ok(false);
    }
    
    let pid = read_pid(pid_file)?;
    
    // Check if process exists
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        
        // Sending None (signal 0) checks if process exists without actually sending a signal
        Ok(kill(Pid::from_raw(pid), None).is_ok())
    }
    
    #[cfg(not(unix))]
    {
        Ok(true) // Assume running on non-Unix platforms
    }
}

fn read_pid(pid_file: &std::path::Path) -> Result<i32> {
    let contents = std::fs::read_to_string(pid_file)?;
    contents.trim().parse::<i32>()
        .map_err(|e| anyhow::anyhow!("Invalid PID file: {}", e))
}

/// Ensure the API server is running, auto-start if needed
async fn ensure_server_running() -> Result<()> {
    use std::path::PathBuf;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // First, try to connect to the API
    if let Ok(response) = reqwest::get(format!("{}/health", api_url)).await {
        if response.status().is_success() {
            // Server is running
            return Ok(());
        }
    }
    
    // If using a non-local API, can't auto-start
    if !api_url.starts_with("http://localhost") && !api_url.starts_with("http://127.0.0.1") {
        anyhow::bail!("Cannot connect to remote API: {}\nPlease ensure the server is running.", api_url);
    }
    
    // Check if local server should be running
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let toygres_dir = PathBuf::from(home).join(".toygres");
    let pid_file = toygres_dir.join("server.pid");
    let log_file = toygres_dir.join("server.log");
    
    // Check if server process exists
    if is_server_running(&pid_file)? {
        // PID file exists but API not responding - give it more time
        println!("Server is starting up, waiting...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        
        if let Ok(response) = reqwest::get(format!("{}/health", api_url)).await {
            if response.status().is_success() {
                return Ok(());
            }
        }
        
        anyhow::bail!("Server is running but not responding. Check logs: ./toygres server logs");
    }
    
    // Server not running - prompt to start
    println!();
    println!("⚠️  No toygres server found.");
    println!();
    println!("Would you like to start a local server? (Y/n) ");
    
    // Read user input
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    
    let answer = line.trim().to_lowercase();
    if answer.is_empty() || answer == "y" || answer == "yes" {
        println!();
        server_start(8080, false, &pid_file, &log_file).await?;
        
        // Wait for server to be fully ready (can take up to 40 seconds for DB init)
        println!();
        println!("Waiting for server to be ready...");
        for i in 0..60 {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            if let Ok(response) = reqwest::get(format!("{}/health", api_url)).await {
                if response.status().is_success() {
                    println!("✓ Server is ready ({}s)", i + 1);
                    println!();
                    return Ok(());
                }
            }
        }
        
        anyhow::bail!("Server started but did not become ready in time. Check logs: ./toygres server logs");
    } else {
        anyhow::bail!("Server is required for this command. Start it with: ./toygres server start");
    }
}

async fn server_orchestrations(status: Option<String>, limit: usize) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/server/orchestrations", api_url))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let mut orchestrations: Vec<serde_json::Value> = response.json().await?;
    
    // Filter by status if provided
    if let Some(status_filter) = &status {
        orchestrations.retain(|o| {
            o["status"].as_str().map(|s| s.contains(status_filter)).unwrap_or(false)
        });
    }
    
    // Limit results
    orchestrations.truncate(limit);
    
    println!("Orchestrations (Advanced Diagnostics)");
    println!("{}", "=".repeat(110));
    println!();
    println!("{:<35} {:<25} {:<10} {:<10} {:<20}", 
             "ID", "TYPE", "VERSION", "STATUS", "STARTED");
    println!("{}", "-".repeat(110));
    
    for orch in &orchestrations {
        let id = orch["instance_id"].as_str().unwrap_or("-");
        let name = orch["orchestration_name"].as_str().unwrap_or("-");
        let version = orch["orchestration_version"].as_str().unwrap_or("-");
        let status = orch["status"].as_str().unwrap_or("-");
        let created = orch["created_at"].as_str().unwrap_or("-");
        
        // Shorten the name for display
        let short_name = name.split("::").last().unwrap_or(name);
        
        println!("{:<35} {:<25} {:<10} {:<10} {}", 
                 id, short_name, version, status, created);
    }
    
    println!();
    println!("{} orchestration(s) found", orchestrations.len());
    println!();
    println!("Use './toygres server orchestration <ID>' for details");
    
    Ok(())
}

async fn server_orchestration(id: &str, history: bool) -> Result<()> {
    // Ensure server is running (auto-start if needed)
    ensure_server_running().await?;
    
    let api_url = std::env::var("TOYGRES_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let response = reqwest::get(format!("{}/api/server/orchestrations/{}", api_url, id))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to API: {}", e))?;
    
    if response.status() == StatusCode::NOT_FOUND {
        anyhow::bail!("Orchestration '{}' not found", id);
    }
    
    if !response.status().is_success() {
        anyhow::bail!("API error: {}", response.status());
    }
    
    let orch: serde_json::Value = response.json().await?;
    
    println!("Orchestration: {}", id);
    println!("{}", "=".repeat(80));
    println!();
    
    let status = orch["status"].as_str().unwrap_or("-");
    println!("Status:          {}", status);
    println!("Type:            {}", orch["orchestration_name"].as_str().unwrap_or("-"));
    println!("Version:         {}", orch["orchestration_version"].as_str().unwrap_or("-"));
    println!("Execution:       #{}", orch["current_execution_id"].as_i64().unwrap_or(0));
    println!();
    println!("Timeline:");
    println!("  Created:       {}", orch["created_at"].as_str().unwrap_or("-"));
    println!("  Updated:       {}", orch["updated_at"].as_str().unwrap_or("-"));
    println!();
    
    // Show output if available
    if let Some(output_val) = orch.get("output") {
        if !output_val.is_null() {
            println!("Output:");
            if let Some(output_str) = output_val.as_str() {
                // Try to parse as JSON for prettier display
                if let Ok(output_json) = serde_json::from_str::<serde_json::Value>(output_str) {
                    println!("{}", serde_json::to_string_pretty(&output_json).unwrap_or(output_str.to_string()));
                } else {
                    println!("{}", output_str);
                }
            }
            println!();
        }
    }
    
    // Show execution history if --history flag is set
    if history {
        if let Some(history_arr) = orch["history"].as_array() {
            if !history_arr.is_empty() {
                println!("Execution History ({} events):", history_arr.len());
                println!("{}", "-".repeat(80));
                println!();
                
                // Pretty-print the history JSON
                if let Ok(pretty) = serde_json::to_string_pretty(history_arr) {
                    println!("{}", pretty);
                } else {
                    println!("{:?}", history_arr);
                }
                println!();
            } else {
                println!("No execution history available");
                println!();
            }
        }
    } else {
        // Show hint about --history flag
        if let Some(history_arr) = orch["history"].as_array() {
            if !history_arr.is_empty() {
                println!("Use '--history' to see {} execution events", history_arr.len());
                println!();
            }
        }
    }
    
    println!("Use './toygres get <instance>' to check instance status");
    
    Ok(())
}

/// Initialize Duroxide runtime and store
async fn initialize_duroxide() -> Result<(Arc<Runtime>, Arc<PostgresProvider>)> {
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
    
    // Initialize CMS schema and verify tables if using PostgreSQL
    if !db_url.starts_with("sqlite") {
        tracing::info!("Initializing CMS schema");
        initialize_cms_schema(&db_url).await?;
        verify_cms_tables(&db_url).await?;
    }
    
    // Create activity and orchestration registries
    let activities = Arc::new(create_activity_registry());
    let orchestrations = create_orchestration_registry();
    
    // Configure runtime options
    let mut runtime_options = RuntimeOptions::default();
    runtime_options.worker_lock_timeout_secs = 300; // 5 minutes
    
    // Start Duroxide runtime
    tracing::info!("Starting Duroxide runtime with 5-minute activity timeout");
    let runtime = Runtime::start_with_options(
        store.clone(),
        activities,
        orchestrations,
        runtime_options,
    )
    .await;
    
    tracing::info!("✓ Duroxide runtime ready");
    
    Ok((runtime, store))
}

