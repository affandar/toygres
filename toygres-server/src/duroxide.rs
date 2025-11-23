use anyhow::Result;
use duroxide::runtime::{Runtime, RuntimeOptions, ObservabilityConfig, LogFormat};
use duroxide_pg::PostgresProvider;
use std::sync::Arc;
use toygres_orchestrations::registry::{create_activity_registry, create_orchestration_registry};

use crate::db;

/// Initialize Duroxide runtime and store
pub async fn initialize() -> Result<(Arc<Runtime>, Arc<PostgresProvider>)> {
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
        db::initialize_cms_schema(&db_url).await?;
        db::verify_cms_tables(&db_url).await?;
    }
    
    // Create activity and orchestration registries
    let activities = Arc::new(create_activity_registry());
    let orchestrations = create_orchestration_registry();
    
    // Configure runtime options
    let mut runtime_options = RuntimeOptions::default();
    runtime_options.worker_lock_timeout = std::time::Duration::from_secs(300); // 5 minutes
    
    // Configure observability (metrics and structured logging)
    let observability_enabled = std::env::var("DUROXIDE_OBSERVABILITY_ENABLED")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);
    
    if observability_enabled {
        let otel_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string());
        
        let log_format_str = std::env::var("DUROXIDE_LOG_FORMAT")
            .unwrap_or_else(|_| "json".to_string())
            .to_lowercase();
        
        let log_format = match log_format_str.as_str() {
            "compact" => LogFormat::Compact,
            "pretty" => LogFormat::Pretty,
            _ => LogFormat::Json,
        };
        
        runtime_options.observability = ObservabilityConfig {
            metrics_enabled: true,
            metrics_export_endpoint: Some(otel_endpoint.clone()),
            metrics_export_interval_ms: 10000, // 10 seconds
            
            log_format,
            log_level: std::env::var("DUROXIDE_LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
            
            service_name: "toygres".to_string(),
            service_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            
            ..Default::default()
        };
        
        tracing::info!(
            "Duroxide observability enabled: metrics → {}, log_format = {}",
            otel_endpoint,
            log_format_str
        );
    } else {
        tracing::info!("Duroxide observability disabled");
    }
    
    // Start Duroxide runtime
    tracing::info!("Starting Duroxide runtime with 5-minute activity timeout");
    let runtime = Runtime::start_with_options(
        store.clone(),
        activities,
        orchestrations,
        runtime_options,
    )
    .await;
    
    // Initialize the duroxide client for activities that need it (e.g., raise_event)
    let client = Arc::new(duroxide::Client::new(store.clone()));
    toygres_orchestrations::init_duroxide_client(client);
    
    tracing::info!("✓ Duroxide runtime ready");
    
    Ok((runtime, store))
}

