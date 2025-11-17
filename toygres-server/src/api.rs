use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use duroxide::Client;
use duroxide_pg::PostgresProvider;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Shared API state
#[derive(Clone)]
pub struct AppState {
    pub duroxide_client: Arc<Client>,
    pub store: Arc<PostgresProvider>,
}

/// Create the API router
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    
    Router::new()
        .route("/health", get(health_check))
        .route("/api/instances", get(list_instances))
        .route("/api/instances/:name", get(get_instance))
        .layer(cors)
        .with_state(state)
}

/// Start the API server
pub async fn start_server(port: u16, state: AppState) -> Result<()> {
    let app = create_router(state);
    
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    tracing::info!("✓ API server listening on {}", addr);
    
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
    
    Ok(())
}

// ============================================================================
// Health Check
// ============================================================================

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "toygres",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// ============================================================================
// Instances
// ============================================================================

#[derive(Debug, Serialize)]
struct InstanceSummary {
    user_name: String,
    k8s_name: String,
    dns_name: Option<String>,
    state: String,
    health_status: String,
    postgres_version: String,
    storage_size_gb: i32,
    created_at: String,
}

async fn list_instances(
    State(_state): State<AppState>,
) -> Result<Json<Vec<InstanceSummary>>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i32, String)>(
        "SELECT user_name, k8s_name, dns_name, state::text, health_status::text, 
                postgres_version, storage_size_gb, created_at::text
         FROM toygres_cms.instances
         WHERE state != 'deleted'
         ORDER BY created_at DESC"
    )
    .fetch_all(&pool)
    .await
    .context("Failed to query instances")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let instances: Vec<InstanceSummary> = rows
        .into_iter()
        .map(|(user_name, k8s_name, dns_name, state, health_status, postgres_version, storage_size_gb, created_at)| {
            InstanceSummary {
                user_name,
                k8s_name,
                dns_name,
                state,
                health_status,
                postgres_version,
                storage_size_gb,
                created_at,
            }
        })
        .collect();
    
    Ok(Json(instances))
}

async fn get_instance(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (
        String, String, String, Option<String>, String, String, String, i32, bool,
        Option<String>, Option<String>, Option<String>, String, String
    )>(
        "SELECT id::text, user_name, k8s_name, dns_name, state::text, health_status::text,
                postgres_version, storage_size_gb, use_load_balancer,
                ip_connection_string, dns_connection_string, external_ip,
                created_at::text, updated_at::text
         FROM toygres_cms.instances
         WHERE dns_name = $1 AND state != 'deleted'
         LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    match row {
        Some((id, user_name, k8s_name, dns_name, state, health_status, postgres_version,
              storage_size_gb, use_load_balancer, ip_conn, dns_conn, external_ip,
              created_at, updated_at)) => {
            Ok(Json(serde_json::json!({
                "id": id,
                "user_name": user_name,
                "k8s_name": k8s_name,
                "dns_name": dns_name,
                "state": state,
                "health_status": health_status,
                "postgres_version": postgres_version,
                "storage_size_gb": storage_size_gb,
                "use_load_balancer": use_load_balancer,
                "ip_connection_string": ip_conn,
                "dns_connection_string": dns_conn,
                "external_ip": external_ip,
                "created_at": created_at,
                "updated_at": updated_at
            })))
        }
        None => Err(AppError::NotFound(format!("Instance '{}' not found", name)))
    }
}

// ============================================================================
// Error Handling
// ============================================================================

#[derive(Debug)]
enum AppError {
    NotImplemented(String),
    NotFound(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        
        let body = Json(serde_json::json!({
            "error": message
        }));
        
        (status, body).into_response()
    }
}
