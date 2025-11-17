use anyhow::Result;
use duroxide::runtime::{Runtime, RuntimeOptions};
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

