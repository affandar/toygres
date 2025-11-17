use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono;
use duroxide::Client;
use duroxide_pg::PostgresProvider;
use serde::Serialize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Shared API state
#[derive(Clone)]
pub struct AppState {
    pub duroxide_client: Arc<Client>,
    #[allow(dead_code)]  // Will be used when we implement create/delete via API
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
        .route("/api/server/orchestrations", get(list_orchestrations))
        .route("/api/server/orchestrations/:id", get(get_orchestration))
        .route("/api/server/orchestrations/:id/cancel", post(cancel_orchestration))
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
// Orchestrations (Duroxide Diagnostics)
// ============================================================================

#[derive(Debug, Serialize)]
struct OrchestrationSummary {
    instance_id: String,
    orchestration_name: String,
    orchestration_version: Option<String>,
    status: String,
    created_at: String,
}

async fn list_orchestrations(
    State(state): State<AppState>,
) -> Result<Json<Vec<OrchestrationSummary>>, AppError> {
    // Check if management features are available
    if !state.duroxide_client.has_management_capability() {
        return Err(AppError::Internal("Management features not available".to_string()));
    }
    
    // Use Duroxide Client management API to list all instances
    let instance_ids = state.duroxide_client
        .list_all_instances()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list instances: {}", e)))?;
    
    // Get info for each instance
    let mut orchestrations = Vec::new();
    for instance_id in instance_ids.iter().take(50) {  // Limit to 50
        if let Ok(info) = state.duroxide_client.get_instance_info(instance_id).await {
            // Convert timestamp (u64 millis) to RFC3339 string
            let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(info.created_at as i64)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string());
            
            orchestrations.push(OrchestrationSummary {
                instance_id: info.instance_id,
                orchestration_name: info.orchestration_name,
                orchestration_version: Some(info.orchestration_version),
                status: info.status,
                created_at,
            });
        }
    }
    
    Ok(Json(orchestrations))
}

async fn get_orchestration(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Check if management features are available
    if !state.duroxide_client.has_management_capability() {
        // Fall back to basic status check
        let status = state.duroxide_client.get_orchestration_status(&id).await;
        
        let (status_str, output) = match &status {
            duroxide::OrchestrationStatus::Running { .. } => ("Running".to_string(), None),
            duroxide::OrchestrationStatus::Completed { output, .. } => ("Completed".to_string(), Some(output.clone())),
            duroxide::OrchestrationStatus::Failed { details, .. } => ("Failed".to_string(), Some(format!("{:?}", details))),
            duroxide::OrchestrationStatus::NotFound => {
                return Err(AppError::NotFound(format!("Orchestration '{}' not found", id)));
            }
        };
        
        return Ok(Json(serde_json::json!({
            "instance_id": id,
            "status": status_str,
            "output": output,
        })));
    }
    
    // Use rich management API to get detailed instance info
    let info = state.duroxide_client
        .get_instance_info(&id)
        .await
        .map_err(|e| {
            if e.contains("not found") {
                AppError::NotFound(format!("Orchestration '{}' not found", id))
            } else {
                AppError::Internal(format!("Failed to get instance info: {}", e))
            }
        })?;
    
    // Convert timestamps (u64 millis) to RFC3339
    let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(info.created_at as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());
    let updated_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(info.updated_at as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());
    
    // Get output if the orchestration completed or failed
    let output = if info.status == "Completed" || info.status == "Failed" {
        // Use get_orchestration_status to get the output
        let status = state.duroxide_client.get_orchestration_status(&id).await;
        match status {
            duroxide::OrchestrationStatus::Completed { output, .. } => Some(output),
            duroxide::OrchestrationStatus::Failed { details, .. } => Some(format!("{:?}", details)),
            _ => None,
        }
    } else {
        None
    };
    
    // Get execution history
    let mut history = Vec::new();
    if let Ok(execution_ids) = state.duroxide_client.list_executions(&id).await {
        for exec_id in execution_ids {
            if let Ok(events) = state.duroxide_client.read_execution_history(&id, exec_id).await {
                for event in events {
                    history.push(serde_json::json!({
                        "execution_id": exec_id,
                        "event": format!("{:?}", event),
                    }));
                }
            }
        }
    }
    
    Ok(Json(serde_json::json!({
        "instance_id": info.instance_id,
        "orchestration_name": info.orchestration_name,
        "orchestration_version": info.orchestration_version,
        "status": info.status,
        "current_execution_id": info.current_execution_id,
        "created_at": created_at,
        "updated_at": updated_at,
        "output": output,
        "history": history,
    })))
}

async fn cancel_orchestration(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Implement once duroxide Client supports cancel_orchestration
    // The cancel_orchestration method currently only exists on OrchestrationContext (for use within orchestrations),
    // not on the Client (for management operations). Once the duroxide management API is extended,
    // we can uncomment the implementation below:
    //
    // if !state.duroxide_client.has_management_capability() {
    //     return Err(AppError::Internal("Management features not available".to_string()));
    // }
    // 
    // state.duroxide_client
    //     .cancel_orchestration(&id)
    //     .await
    //     .map_err(|e| AppError::Internal(format!("Failed to cancel: {}", e)))?;
    
    Err(AppError::NotImplemented(
        "Orchestration cancellation via management API not yet available in duroxide. \
         This feature requires duroxide Client to expose a cancel_orchestration method.".to_string()
    ))
}

// ============================================================================
// Error Handling
// ============================================================================

#[derive(Debug)]
enum AppError {
    #[allow(dead_code)]  // Will be used when we add create/delete endpoints
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
