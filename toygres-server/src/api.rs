use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
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
        .route("/api/instances", get(list_instances).post(create_instance))
        .route("/api/instances/bulk", post(bulk_create_instances))
        .route("/api/instances/bulk/delete", post(bulk_delete_instances))
        .route("/api/instances/:name", get(get_instance).delete(delete_instance))
        .route("/api/server/orchestrations", get(list_orchestrations))
        .route("/api/server/orchestrations/:id", get(get_orchestration))
        .route("/api/server/orchestrations/:id/cancel", post(cancel_orchestration))
        .route("/api/server/orchestrations/:id/recreate", post(recreate_orchestration))
        .route("/api/server/orchestrations/:id/raise-event", post(raise_event_to_orchestration))
        .route("/api/server/orchestration-flows", get(list_orchestration_flows))
        .route("/api/server/orchestration-flows/:name", get(get_orchestration_flow))
        .route("/api/server/logs", get(get_logs))
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

#[derive(Debug, serde::Deserialize)]
struct CreateInstanceRequest {
    name: String,
    password: String,
    #[serde(default = "default_version")]
    postgres_version: String,
    #[serde(default = "default_storage")]
    storage_size_gb: i32,
    #[serde(default)]
    internal: bool,
    #[serde(default = "default_namespace")]
    namespace: String,
}

fn default_version() -> String {
    "18".to_string()
}

fn default_storage() -> i32 {
    10
}

fn default_namespace() -> String {
    "toygres".to_string()
}

async fn create_instance(
    State(state): State<AppState>,
    Json(req): Json<CreateInstanceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use uuid::Uuid;
    use toygres_orchestrations::types::CreateInstanceInput;
    
    // Validate name
    if req.name.is_empty() || !req.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::BadRequest("Invalid instance name. Use only alphanumeric characters and hyphens.".to_string()));
    }
    
    if req.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".to_string()));
    }
    
    // Generate K8s name (name + random suffix)
    let suffix = Uuid::new_v4().to_string().split('-').next().unwrap().to_string();
    let k8s_name = format!("{}-{}", req.name, suffix);
    let orchestration_id = format!("create-{}", k8s_name);
    
    let input = CreateInstanceInput {
        user_name: req.name.clone(),
        name: k8s_name.clone(),
        password: req.password,
        postgres_version: Some(req.postgres_version),
        storage_size_gb: Some(req.storage_size_gb),
        use_load_balancer: Some(!req.internal),
        dns_label: Some(req.name.clone()),
        namespace: Some(req.namespace),
        orchestration_id: orchestration_id.clone(),
    };
    
    // Start the create orchestration
    state.duroxide_client
        .start_orchestration(
            &orchestration_id,
            toygres_orchestrations::names::orchestrations::CREATE_INSTANCE,
            &serde_json::to_string(&input).unwrap(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start orchestration: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": req.name,
        "k8s_name": k8s_name,
        "orchestration_id": orchestration_id,
        "dns_name": format!("{}.westus3.cloudapp.azure.com", req.name),
    })))
}

async fn bulk_create_instances(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    use uuid::Uuid;
    use toygres_orchestrations::types::CreateInstanceInput;
    
    let base_name = req.get("base_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing base_name".to_string()))?;
    
    let count = req.get("count")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::BadRequest("Missing count".to_string()))? as usize;
    
    let password = req.get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing password".to_string()))?;
    
    let postgres_version = req.get("postgres_version")
        .and_then(|v| v.as_str())
        .unwrap_or("18");
    
    let storage_size_gb = req.get("storage_size_gb")
        .and_then(|v| v.as_i64())
        .unwrap_or(10) as i32;
    
    let internal = req.get("internal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    let namespace = req.get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("toygres");
    
    // Validate
    if base_name.is_empty() || !base_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::BadRequest("Invalid base name. Use only alphanumeric characters and hyphens.".to_string()));
    }
    
    if count == 0 || count > 50 {
        return Err(AppError::BadRequest("Count must be between 1 and 50".to_string()));
    }
    
    if password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".to_string()));
    }
    
    let mut created_instances = Vec::new();
    
    for i in 1..=count {
        let user_name = format!("{}{}", base_name, i);
        let suffix = Uuid::new_v4().to_string().split('-').next().unwrap().to_string();
        let k8s_name = format!("{}-{}", user_name, suffix);
        let orchestration_id = format!("create-{}", k8s_name);
        
        let input = CreateInstanceInput {
            user_name: user_name.clone(),
            name: k8s_name.clone(),
            password: password.to_string(),
            postgres_version: Some(postgres_version.to_string()),
            storage_size_gb: Some(storage_size_gb),
            use_load_balancer: Some(!internal),
            dns_label: Some(user_name.clone()),
            namespace: Some(namespace.to_string()),
            orchestration_id: orchestration_id.clone(),
        };
        
        state.duroxide_client
            .start_orchestration(
                &orchestration_id,
                toygres_orchestrations::names::orchestrations::CREATE_INSTANCE,
                &serde_json::to_string(&input).unwrap(),
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to start orchestration {}: {}", i, e)))?;
        
        created_instances.push(serde_json::json!({
            "instance_name": user_name,
            "k8s_name": k8s_name,
            "orchestration_id": orchestration_id,
            "dns_name": format!("{}.westus3.cloudapp.azure.com", user_name),
        }));
    }
    
    Ok(Json(serde_json::json!({
        "count": count,
        "instances": created_instances,
    })))
}

async fn bulk_delete_instances(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::types::DeleteInstanceInput;
    
    let instance_names = req.get("instance_names")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("Missing instance_names array".to_string()))?;
    
    if instance_names.is_empty() || instance_names.len() > 50 {
        return Err(AppError::BadRequest("instance_names must contain 1-50 items".to_string()));
    }
    
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let mut deleted_instances = Vec::new();
    let mut errors = Vec::new();
    
    for name_val in instance_names {
        let name = name_val.as_str()
            .ok_or_else(|| AppError::BadRequest("Invalid instance name in array".to_string()))?;
        
        // Get the k8s name for this instance
        let result = sqlx::query_scalar::<_, String>(
            "SELECT k8s_name FROM toygres_cms.instances WHERE user_name = $1"
        )
        .bind(name)
        .fetch_optional(&pool)
        .await
        .context("Failed to query instance")
        .map_err(|e| AppError::Internal(e.to_string()))?;
        
        match result {
            Some(k8s_name) => {
                let orchestration_id = format!("delete-{}", k8s_name);
                
                let input = DeleteInstanceInput {
                    name: k8s_name.clone(),
                    namespace: Some("toygres".to_string()),
                    orchestration_id: orchestration_id.clone(),
                };
                
                match state.duroxide_client
                    .start_orchestration(
                        &orchestration_id,
                        toygres_orchestrations::names::orchestrations::DELETE_INSTANCE,
                        &serde_json::to_string(&input).unwrap(),
                    )
                    .await
                {
                    Ok(_) => {
                        deleted_instances.push(serde_json::json!({
                            "instance_name": name,
                            "k8s_name": k8s_name,
                            "orchestration_id": orchestration_id,
                        }));
                    }
                    Err(e) => {
                        errors.push(serde_json::json!({
                            "instance_name": name,
                            "error": e.to_string(),
                        }));
                    }
                }
            }
            None => {
                errors.push(serde_json::json!({
                    "instance_name": name,
                    "error": "Instance not found",
                }));
            }
        }
    }
    
    Ok(Json(serde_json::json!({
        "deleted": deleted_instances.len(),
        "errors": errors.len(),
        "instances": deleted_instances,
        "failures": errors,
    })))
}

async fn delete_instance(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::types::DeleteInstanceInput;
    
    // Look up the instance by name
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT k8s_name, namespace FROM toygres_cms.instances WHERE dns_name = $1 AND state != 'deleted' LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, namespace) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found or already deleted", name))),
    };
    
    let orchestration_id = format!("delete-{}", k8s_name);
    
    let input = DeleteInstanceInput {
        name: k8s_name.clone(),
        namespace: Some(namespace),
        orchestration_id: orchestration_id.clone(),
    };
    
    // Start the delete orchestration
    state.duroxide_client
        .start_orchestration(
            &orchestration_id,
            toygres_orchestrations::names::orchestrations::DELETE_INSTANCE,
            &serde_json::to_string(&input).unwrap(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start delete orchestration: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "orchestration_id": orchestration_id,
    })))
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
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Check if management features are available
    if !state.duroxide_client.has_management_capability() {
        // Fall back to basic status check
        let status = state.duroxide_client.get_orchestration_status(&id).await
            .map_err(|e| AppError::Internal(format!("Failed to get orchestration status: {}", e)))?;
        
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
            let error_msg = format!("{:?}", e);
            if error_msg.contains("not found") || error_msg.contains("NotFound") {
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
        let status = state.duroxide_client.get_orchestration_status(&id).await
            .map_err(|e| AppError::Internal(format!("Failed to get orchestration status: {}", e)))?;
        match status {
            duroxide::OrchestrationStatus::Completed { output, .. } => Some(output),
            duroxide::OrchestrationStatus::Failed { details, .. } => Some(format!("{:?}", details)),
            _ => None,
        }
    } else {
        None
    };
    
    // Get execution history with optional limit
    let mut history = Vec::new();
    if let Ok(execution_ids) = state.duroxide_client.list_executions(&id).await {
        // Parse history_limit from query params: "full", "5", or "10"
        let limit = params.get("history_limit")
            .and_then(|v| {
                if v == "full" {
                    Some(None)
                } else {
                    v.parse::<usize>().ok().map(Some)
                }
            })
            .flatten();
        
        let execution_ids_to_process = if let Some(limit) = limit {
            // Take only the last N executions
            let start_idx = execution_ids.len().saturating_sub(limit);
            &execution_ids[start_idx..]
        } else {
            // Full history
            &execution_ids[..]
        };
        
        for exec_id in execution_ids_to_process {
            if let Ok(events) = state.duroxide_client.read_execution_history(&id, *exec_id).await {
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

async fn raise_event_to_orchestration(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let event_name = req.get("event_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing event_name".to_string()))?;
    
    let event_data = req.get("event_data")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    
    state.duroxide_client
        .raise_event(&id, event_name, event_data)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to raise event: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "instance_id": id,
        "event_name": event_name,
        "raised": true,
    })))
}

async fn recreate_orchestration(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Get the orchestration details to extract input and metadata
    if !state.duroxide_client.has_management_capability() {
        return Err(AppError::Internal("Management features not available".to_string()));
    }
    
    let info = state.duroxide_client
        .get_instance_info(&id)
        .await
        .map_err(|e| AppError::NotFound(format!("Orchestration not found: {}", e)))?;
    
    // Extract orchestration name and version
    let orch_name = info.orchestration_name;
    let orch_version = info.orchestration_version;
    
    // Get the input from history (it's in the OrchestrationStarted event)
    let execution_ids = state.duroxide_client
        .list_executions(&id)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list executions: {}", e)))?;
    
    let first_exec = execution_ids.first()
        .ok_or_else(|| AppError::Internal("No executions found".to_string()))?;
    
    let events = state.duroxide_client
        .read_execution_history(&id, *first_exec)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read history: {}", e)))?;
    
    let input = events.iter()
        .find_map(|event| {
            if let duroxide::EventKind::OrchestrationStarted { input, .. } = &event.kind {
                Some(input.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| AppError::Internal("Could not find input in orchestration history".to_string()))?;
    
    // Generate a new instance ID based on the orchestration type
    use uuid::Uuid;
    let new_suffix = Uuid::new_v4().to_string().split('-').next().unwrap().to_string();
    
    // Extract the base name from the original ID (e.g., "create-mydb-abc123" -> "mydb")
    let base_parts: Vec<&str> = id.split('-').collect();
    let new_id = if base_parts.len() >= 2 {
        // Has format like "create-name-guid" or "actor-name-guid"
        let prefix = base_parts[0];
        let name_parts = &base_parts[1..base_parts.len()-1];
        let name = name_parts.join("-");
        format!("{}-{}-{}", prefix, name, new_suffix)
    } else {
        // Fallback: just append new suffix
        format!("{}-recreate-{}", id, new_suffix)
    };
    
    // Start the new orchestration with the same parameters
    state.duroxide_client
        .start_orchestration_versioned(
            &new_id,
            &orch_name,
            &orch_version,
            &input,
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start orchestration: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "new_instance_id": new_id,
        "original_instance_id": id,
        "orchestration_name": orch_name,
        "orchestration_version": orch_version,
    })))
}

// ============================================================================
// Orchestration Flows (Static Diagrams)
// ============================================================================

async fn list_orchestration_flows() -> Result<Json<Vec<serde_json::Value>>, AppError> {
    use toygres_orchestrations::flows;
    
    let all_flows = flows::get_all_flows();
    let result: Vec<serde_json::Value> = all_flows
        .iter()
        .map(|flow| {
            serde_json::json!({
                "orchestration_name": flow.orchestration_name,
                "mermaid": flow.mermaid,
                "node_mappings": flow.node_mappings.iter()
                    .map(|(node_id, activity_pattern)| {
                        serde_json::json!({
                            "node_id": node_id,
                            "activity_pattern": activity_pattern,
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    
    Ok(Json(result))
}

async fn get_orchestration_flow(
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use toygres_orchestrations::flows;
    
    let flow = flows::get_flow_by_name(&name)
        .ok_or_else(|| AppError::NotFound(format!("Flow for '{}' not found", name)))?;
    
    Ok(Json(serde_json::json!({
        "orchestration_name": flow.orchestration_name,
        "mermaid": flow.mermaid,
        "node_mappings": flow.node_mappings.iter()
            .map(|(node_id, activity_pattern)| {
                serde_json::json!({
                    "node_id": node_id,
                    "activity_pattern": activity_pattern,
                })
            })
            .collect::<Vec<_>>(),
    })))
}

// ============================================================================
// Server Logs
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct LogsQuery {
    #[serde(default = "default_log_limit")]
    limit: usize,
    #[serde(default)]
    filter: Option<String>,
}

fn default_log_limit() -> usize {
    200
}

async fn get_logs(
    State(_state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<Vec<String>>, AppError> {
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let log_file = PathBuf::from(home).join(".toygres").join("server.log");
    
    if !log_file.exists() {
        return Ok(Json(vec![]));
    }
    
    let file = std::fs::File::open(&log_file)
        .map_err(|e| AppError::Internal(format!("Failed to open log file: {}", e)))?;
    
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = reader
        .lines()
        .filter_map(|l| l.ok())
        .collect();
    
    // Apply filter if provided
    if let Some(ref filter) = query.filter {
        lines.retain(|line| line.contains(filter));
    }
    
    // Take last N lines
    let start = if lines.len() > query.limit {
        lines.len() - query.limit
    } else {
        0
    };
    
    Ok(Json(lines[start..].to_vec()))
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
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };
        
        let body = Json(serde_json::json!({
            "error": message
        }));
        
        (status, body).into_response()
    }
}
