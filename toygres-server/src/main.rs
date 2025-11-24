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

/// Initialize tracing with dual output:
/// 1. OTLP export to OpenTelemetry Collector → Loki (primary)
/// 2. File output (~/.toygres/server.log) - JSON format backup
fn initialize_tracing() -> Result<()> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;
    use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
    use opentelemetry_sdk::logs::LoggerProvider as SdkLoggerProvider;
    use opentelemetry_sdk::Resource;
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    
    // Create the base filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default: debug level for toygres and duroxide
            "toygres_server=debug,\
             toygres_activities=debug,\
             toygres_orchestrations=debug,\
             duroxide=debug,\
             duroxide_pg=debug"
                .into()
        });
    
    // Set up file logging to ~/.toygres/server.log (backup)
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let toygres_dir = PathBuf::from(home).join(".toygres");
    std::fs::create_dir_all(&toygres_dir).ok();
    
    let file_appender = tracing_appender::rolling::never(&toygres_dir, "server.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Check if OTLP is enabled
    let otlp_enabled = std::env::var("DUROXIDE_OBSERVABILITY_ENABLED")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);
    
    if otlp_enabled {
        let otel_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string());
        
        // Create OTLP log exporter
        let exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(&otel_endpoint)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create OTLP log exporter: {}", e))?;
        
        // Create logger provider
        let logger_provider = SdkLoggerProvider::builder()
            .with_resource(Resource::new(vec![
                KeyValue::new("service.name", "toygres"),
                KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            ]))
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .build();
        
        // Create OpenTelemetry tracing layer
        let otel_layer = OpenTelemetryTracingBridge::new(&logger_provider);
        
        // File layer: JSON format (backup for debugging)
        let file_layer = fmt::layer()
            .with_writer(file_writer)
            .with_ansi(false)
            .json();
        
        // Initialize with OTLP and file layers
        tracing_subscriber::registry()
            .with(env_filter)
            .with(otel_layer)
            .with(file_layer)
            .init();
        
        eprintln!("✓ Tracing initialized with OTLP export to {}", otel_endpoint);
        eprintln!("  - OTLP: {} → OTEL Collector → Loki", otel_endpoint);
        eprintln!("  - File: ~/.toygres/server.log (JSON backup)");
        
        // Store the logger provider to prevent early drop
        std::mem::forget(logger_provider);
    } else {
        // File layer: JSON format
        let file_layer = fmt::layer()
            .with_writer(file_writer)
            .with_ansi(false)
            .json();
        
        // Initialize without OTLP
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
        
        eprintln!("✓ Tracing initialized (OTLP disabled)");
        eprintln!("  - File: ~/.toygres/server.log (JSON)");
    }
    
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
