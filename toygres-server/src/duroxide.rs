use anyhow::Result;
use duroxide::runtime::{Runtime, RuntimeOptions, ObservabilityConfig, LogFormat};
use duroxide_pg::PostgresProvider;
use metrics_exporter_prometheus::PrometheusBuilder;
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
    let activities = create_activity_registry();
    let orchestrations = create_orchestration_registry();

    // Log all registered orchestrations
    let orch_names = orchestrations.list_names();
    tracing::info!("Registered {} orchestrations:", orch_names.len());
    for name in &orch_names {
        let versions = orchestrations.list_versions(name);
        tracing::info!("  - {} (versions: {:?})", name, versions);
    }
    
    // Configure runtime options
    let mut runtime_options = RuntimeOptions::default();
    runtime_options.dispatcher_min_poll_interval = std::time::Duration::from_secs(1); // 1 second polling (default: 100ms)
    runtime_options.orchestration_concurrency = 2;   // 2 orchestration workers per replica
    runtime_options.worker_concurrency = 10;         // 10 activity workers per replica
    runtime_options.worker_lock_timeout = std::time::Duration::from_secs(300); // 5 minutes for activities
    runtime_options.orchestrator_lock_timeout = std::time::Duration::from_secs(30); // 30 seconds for orchestrations (default: 5s)
    runtime_options.orchestrator_lock_renewal_buffer = std::time::Duration::from_secs(5); // Renew 5s before expiry
    
    // Install Prometheus metrics exporter (exposes /metrics on port 9091)
    let metrics_port: u16 = std::env::var("METRICS_PORT")
        .unwrap_or_else(|_| "9091".to_string())
        .parse()
        .unwrap_or(9091);
    
    match PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], metrics_port))
        .install()
    {
        Ok(()) => {
            tracing::info!("Prometheus metrics exporter started on port {}", metrics_port);
        }
        Err(e) => {
            tracing::warn!("Failed to start Prometheus metrics exporter: {} (metrics will not be available)", e);
        }
    }
    
    // Configure observability (structured logging)
    let log_format_str = std::env::var("DUROXIDE_LOG_FORMAT")
        .unwrap_or_else(|_| "json".to_string())
        .to_lowercase();
    
    let log_format = match log_format_str.as_str() {
        "compact" => LogFormat::Compact,
        "pretty" => LogFormat::Pretty,
        _ => LogFormat::Json,
    };
    
    runtime_options.observability = ObservabilityConfig {
        log_format,
        log_level: std::env::var("DUROXIDE_LOG_LEVEL")
            .unwrap_or_else(|_| "debug".to_string()),
        service_name: "toygres".to_string(),
        service_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        ..Default::default()
    };
    
    tracing::info!(
        "Duroxide observability configured: log_format = {}, metrics at :{}/metrics",
        log_format_str,
        metrics_port
    );
    
    // Start Duroxide runtime
    tracing::info!("Starting Duroxide runtime: 10 orchestration workers, 10 activity workers, 5-minute activity timeout");
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

