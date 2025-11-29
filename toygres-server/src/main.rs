use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::path::PathBuf;

mod api;
mod cli;
mod commands;
mod config;
mod db;
mod duroxide;
mod worker;

use cli::{Args, Mode};

/// Initialize tracing with dual output for development:
/// 1. Console output (stdout) - for CLI logs viewing
/// 2. File output (~/.toygres/server.log) - JSON format for persistence
fn initialize_tracing() -> Result<()> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;
    
    // Create the base filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default: debug level for all components
            // NOTE: duroxide::runtime must be explicitly enabled for worker logs
            "debug,\
             toygres_server=debug,\
             toygres_orchestrations=debug,\
             duroxide=debug,\
             duroxide::runtime=debug,\
             duroxide::runtime::dispatchers=debug,\
             duroxide_pg=debug,\
             sqlx::query=warn"
                .into()
        });
    
    // Set up file logging to ~/.toygres/server.log
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let toygres_dir = PathBuf::from(home).join(".toygres");
    std::fs::create_dir_all(&toygres_dir).ok();
    
    let file_appender = tracing_appender::rolling::never(&toygres_dir, "server.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    
    // CRITICAL: Keep guard alive for the lifetime of the program
    // Dropping the guard will stop file logging
    std::mem::forget(guard);
    
    // File layer: Flat text format with ANSI colors
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(true);
    
    // Initialize with file layer only
    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .init();
    
    eprintln!("✓ Tracing initialized");
    eprintln!("  - File: ~/.toygres/server.log (flat text with colors)");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize tracing with multiple outputs
    initialize_tracing()?;

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
