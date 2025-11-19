use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod cli;
mod commands;
mod config;
mod db;
mod duroxide;
mod worker;

use cli::{Args, Mode};

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
                    // Default: debug level for toygres and duroxide
                    "toygres_server=debug,\
                     toygres_activities=debug,\
                     toygres_orchestrations=debug,\
                     duroxide=debug,\
                     duroxide_pg=debug"
                        .into()
                }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Route to appropriate handler
    match args.mode {
        Mode::Standalone { port, workers } => {
            commands::server::run_standalone_mode(port, workers).await
        }
        Mode::Api { port } => {
            run_api_mode(port).await
        }
        Mode::Worker { worker_id } => {
            run_worker_mode(worker_id).await
        }
        Mode::Create { name, password, version, storage, internal, namespace } => {
            commands::instance::run_create(name, password, version, storage, internal, namespace).await
        }
        Mode::Delete { name, namespace } => {
            commands::instance::run_delete(name, namespace).await
        }
        Mode::List { output } => {
            commands::instance::run_list(output).await
        }
        Mode::Get { name, output } => {
            commands::instance::run_get(name, output).await
        }
        Mode::Server { command } => {
            commands::server::handle_command(command).await
        }
    }
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
    use uuid::Uuid;
    
    let id = worker_id.unwrap_or_else(|| format!("worker-{}", Uuid::new_v4()));
    tracing::info!("Starting Toygres in worker-only mode");
    tracing::info!("Worker ID: {}", id);
    
    // TODO: Implement worker mode
    // - Start Duroxide runtime with workers
    // - No API server
    
    anyhow::bail!("Worker mode not yet implemented")
}
