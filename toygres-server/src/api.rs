use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use duroxide::{Client, EventKind};
use duroxide_pg::PostgresProvider;
use serde::Serialize;
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;
use tower_http::cors::{Any, CorsLayer};

use crate::auth;

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
        // Auth routes
        .route("/login", get(auth::login_page).post(auth::login_handler))
        .route("/logout", post(auth::logout_handler))
        // Health check (public)
        .route("/health", get(health_check))
        // API routes (protected)
        .route("/api/instances", get(list_instances).post(create_instance))
        .route("/api/instances/bulk", post(bulk_create_instances))
        .route("/api/instances/bulk/delete", post(bulk_delete_instances))
        .route("/api/instances/:name", get(get_instance).delete(delete_instance))
        .route("/api/instances/:name/stop", post(stop_instance))
        .route("/api/instances/:name/start", post(start_instance))
        .route("/api/instances/:name/restart", post(restart_instance))
        .route("/api/instances/:name/actor/start", post(start_instance_actor))
        .route("/api/instances/:name/actor/restart", post(restart_instance_actor))
        .route("/api/instances/:name/actor/cancel", post(cancel_instance_actor))
        .route("/api/instances/:name/logs", get(get_instance_logs))
        .route("/api/instances/:name/durable-orchestrations", get(get_pg_durable_orchestrations))
        .route("/api/instances/:name/durable-orchestrations/:instance_id/nodes", get(get_pg_durable_instance_nodes))
        .route("/api/instances/:name/durable-orchestrations/:instance_id/explain", get(get_pg_durable_explain))
        .route("/api/server/orchestrations", get(list_orchestrations))
        .route("/api/server/orchestrations/:id", get(get_orchestration))
        .route("/api/server/orchestrations/:id/cancel", post(cancel_orchestration))
        .route("/api/server/orchestrations/:id/recreate", post(recreate_orchestration))
        .route("/api/server/orchestrations/:id/raise-event", post(raise_event_to_orchestration))
        .route("/api/server/orchestrations/:id/delete", post(delete_orchestration_instance))
        .route("/api/server/orchestrations/:id/prune", post(prune_orchestration))
        .route("/api/server/orchestrations/:id/tree", get(get_orchestration_tree))
        .route("/api/server/orchestration-flows", get(list_orchestration_flows))
        .route("/api/server/orchestration-flows/:name", get(get_orchestration_flow))
        .route("/api/server/logs", get(get_logs))
        .route("/api/server/prune-log", get(get_prune_log))
        // Image routes
        .route("/api/images", get(list_images).post(create_image))
        .route("/api/images/:name", get(get_image).delete(delete_image))
        .route("/api/images/:name/logs", get(get_image_job_logs))
        .route("/api/instances/:name/images", post(create_image_from_instance))
        // Runtime image catalog routes (ACR OCI images)
        .route("/api/runtime-images", get(list_runtime_images))
        .route("/api/runtime-images/register", post(register_runtime_image))
        // Auth middleware
        .layer(middleware::from_fn(auth::auth_middleware))
        // Cookie management
        .layer(CookieManagerLayer::new())
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

/// Extract hostname from a PostgreSQL connection string
/// Input: postgresql://user:password@host:port/db or postgresql://user:password@host/db
/// Output: host
fn extract_db_hostname(db_url: &str) -> String {
    // Try to parse: postgresql://user:pass@host:port/db or postgresql://user:pass@host/db
    if let Some(rest) = db_url.strip_prefix("postgresql://") {
        if let Some(at_pos) = rest.find('@') {
            let host_part = &rest[at_pos + 1..]; // everything after @
            // Find the end of hostname (: for port or / for database)
            let end_pos = host_part.find(':').or_else(|| host_part.find('/')).unwrap_or(host_part.len());
            return host_part[..end_pos].to_string();
        }
    }
    "unknown".to_string()
}

async fn health_check() -> impl IntoResponse {
    // Get database URL and extract hostname
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "not configured".to_string());
    let db_hostname = extract_db_hostname(&db_url);
    
    // CMS and Duroxide use the same database server, just different schemas
    Json(serde_json::json!({
        "status": "healthy",
        "service": "toygres",
        "version": env!("CARGO_PKG_VERSION"),
        "cms_db_hostname": db_hostname,
        "duroxide_db_hostname": db_hostname
    }))
}

// ============================================================================
// Instances
// ============================================================================

/// URL encode the password portion of a PostgreSQL connection string
/// Input: postgresql://user:password@host:port/db
/// Output: postgresql://user:encoded_password@host:port/db
fn url_encode_connection_string_password(conn_str: &str) -> String {
    // Parse: postgresql://user:password@host:port/db
    if let Some(rest) = conn_str.strip_prefix("postgresql://") {
        if let Some(at_pos) = rest.find('@') {
            let user_pass = &rest[..at_pos];
            let host_db = &rest[at_pos..];
            
            if let Some(colon_pos) = user_pass.find(':') {
                let user = &user_pass[..colon_pos];
                let password = &user_pass[colon_pos + 1..];
                
                // URL encode the password
                let encoded_password = url_encode_password(password);
                
                return format!("postgresql://{}:{}{}",user, encoded_password, host_db);
            }
        }
    }
    // Return original if parsing fails
    conn_str.to_string()
}

/// URL encode a password for use in connection strings
fn url_encode_password(password: &str) -> String {
    let mut encoded = String::with_capacity(password.len() * 3);
    for c in password.chars() {
        match c {
            // Safe characters (alphanumeric and a few others)
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                encoded.push(c);
            }
            // Everything else needs encoding
            _ => {
                for byte in c.to_string().as_bytes() {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    encoded
}

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
    image_type: String,
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
    
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i32, String, String)>(
        "SELECT user_name, k8s_name, dns_name, state::text, health_status::text, 
                postgres_version, storage_size_gb, created_at::text,
                COALESCE(image_type::text, 'stock') as image_type
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
        .map(|(user_name, k8s_name, dns_name, state, health_status, postgres_version, storage_size_gb, created_at, image_type)| {
            InstanceSummary {
                user_name,
                k8s_name,
                dns_name,
                state,
                health_status,
                postgres_version,
                storage_size_gb,
                created_at,
                image_type,
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
    
    // Use raw query to avoid tuple size limitations
    let row = sqlx::query(
        "SELECT id::text, user_name, k8s_name, dns_name, state::text, health_status::text,
                postgres_version, storage_size_gb, use_load_balancer,
                ip_connection_string, dns_connection_string, external_ip,
                created_at::text, updated_at::text,
                create_orchestration_id, instance_actor_orchestration_id,
                COALESCE(image_type::text, 'stock') as image_type,
                last_health_check::text
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
        Some(row) => {
            use sqlx::Row;
            // URL encode passwords in connection strings for safe copy-paste
            let ip_conn: Option<String> = row.get("ip_connection_string");
            let dns_conn: Option<String> = row.get("dns_connection_string");
            let ip_conn_encoded = ip_conn.map(|s| url_encode_connection_string_password(&s));
            let dns_conn_encoded = dns_conn.map(|s| url_encode_connection_string_password(&s));
            
            Ok(Json(serde_json::json!({
                "id": row.get::<String, _>("id"),
                "user_name": row.get::<String, _>("user_name"),
                "k8s_name": row.get::<String, _>("k8s_name"),
                "dns_name": row.get::<Option<String>, _>("dns_name"),
                "state": row.get::<String, _>("state"),
                "health_status": row.get::<String, _>("health_status"),
                "postgres_version": row.get::<String, _>("postgres_version"),
                "storage_size_gb": row.get::<i32, _>("storage_size_gb"),
                "use_load_balancer": row.get::<bool, _>("use_load_balancer"),
                "ip_connection_string": ip_conn_encoded,
                "dns_connection_string": dns_conn_encoded,
                "external_ip": row.get::<Option<String>, _>("external_ip"),
                "created_at": row.get::<String, _>("created_at"),
                "updated_at": row.get::<String, _>("updated_at"),
                "create_orchestration_id": row.get::<Option<String>, _>("create_orchestration_id"),
                "instance_actor_orchestration_id": row.get::<Option<String>, _>("instance_actor_orchestration_id"),
                "image_type": row.get::<String, _>("image_type"),
                "last_health_check": row.get::<Option<String>, _>("last_health_check")
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
    /// Image type: "stock" (default) or "pg_durable"
    #[serde(default = "default_image_type")]
    image_type: String,
    /// Optional source image ID to restore from
    #[serde(default)]
    source_image_id: Option<String>,

    /// Optional runtime image ID (ACR OCI image) to deploy from
    #[serde(default)]
    runtime_image_id: Option<String>,
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

fn default_image_type() -> String {
    "stock".to_string()
}

async fn create_instance(
    State(state): State<AppState>,
    Json(req): Json<CreateInstanceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use uuid::Uuid;
    use toygres_orchestrations::types::CreateInstanceInput;
    use toygres_orchestrations::activity_types::ImageType;
    
    // Validate name
    if req.name.is_empty() || !req.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::BadRequest("Invalid instance name. Use only alphanumeric characters and hyphens.".to_string()));
    }
    
    if req.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".to_string()));
    }
    
    // Parse image type. Note: runtime images are always deployed in stock mode.
    let mut image_type = ImageType::from_str(&req.image_type);

    // If a runtime image is specified, resolve it to a canonical pull ref
    let (runtime_image_id, image_override) = if let Some(runtime_image_id) = &req.runtime_image_id {
        // Runtime images are treated as stock deployments (pg_durable is built-in only).
        image_type = ImageType::Stock;

        let image_uuid = Uuid::parse_str(runtime_image_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid runtime_image_id: {}", e)))?;

        let pool = get_cms_pool().await
            .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

        let row = sqlx::query(
            r#"
            SELECT acr_ref, digest
            FROM toygres_cms.runtime_images
            WHERE id = $1 AND state != 'deleted'
            "#
        )
        .bind(image_uuid)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to resolve runtime image: {}", e)))?;

        let (acr_ref, digest): (String, String) = match row {
            Some(row) => {
                use sqlx::Row;
                (row.get("acr_ref"), row.get("digest"))
            }
            None => {
                return Err(AppError::BadRequest("Runtime image not found (or deleted)".to_string()));
            }
        };

        // Store the override as a digest-pinned pull string
        let pull_ref = format!("{}@{}", acr_ref, digest);
        (Some(runtime_image_id.clone()), Some(pull_ref))
    } else {
        (None, None)
    };
    
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
        image_type: image_type.clone(),
        source_image_id: req.source_image_id,
        runtime_image_id,
        image_override,
    };
    
    // Start the create orchestration (uses latest registered version via default Latest policy)
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
        "image_type": image_type.as_str(),
    })))
}

// ============================================================================
// Runtime Image Catalog API Handlers
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct RegisterRuntimeImageRequest {
    name: String,
    acr_ref: String,
    digest: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_image_type")]
    suggested_image_type: String,
}

fn is_valid_sha256_digest(digest: &str) -> bool {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn validate_acr_ref_is_toygres(acr_ref: &str) -> Result<(), AppError> {
    let acr_host = std::env::var("TOYGRES_ACR_HOST").unwrap_or_else(|_| "toygresaksacr.azurecr.io".to_string());

    // Allow either full image ref "host/repo[:tag]" or "host/repo".
    let prefix = format!("{}/", acr_host);
    if !acr_ref.starts_with(&prefix) {
        return Err(AppError::BadRequest(format!(
            "acr_ref must start with '{}' (set TOYGRES_ACR_HOST to override)",
            prefix
        )));
    }

    Ok(())
}

async fn list_runtime_images(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    let rows = sqlx::query(
        r#"
        SELECT id, name, description, acr_ref, digest,
               suggested_image_type::text AS suggested_image_type,
               state, created_at
        FROM toygres_cms.runtime_images
        WHERE state != 'deleted'
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to list runtime images: {}", e)))?;

    let images: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            use chrono::{DateTime, Utc};
            use sqlx::Row;

            let created_at: DateTime<Utc> = row.get("created_at");
            serde_json::json!({
                "id": row.get::<uuid::Uuid, _>("id").to_string(),
                "name": row.get::<String, _>("name"),
                "description": row.get::<Option<String>, _>("description"),
                "acr_ref": row.get::<String, _>("acr_ref"),
                "digest": row.get::<String, _>("digest"),
                "suggested_image_type": row.get::<String, _>("suggested_image_type"),
                "state": row.get::<String, _>("state"),
                "created_at": created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!(images)))
}

async fn register_runtime_image(
    State(_state): State<AppState>,
    Json(req): Json<RegisterRuntimeImageRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use uuid::Uuid;

    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    if !req.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest(
            "Invalid name. Use only alphanumeric characters, hyphens, and underscores.".to_string(),
        ));
    }

    validate_acr_ref_is_toygres(&req.acr_ref)?;

    let digest = req.digest.to_lowercase();
    if !is_valid_sha256_digest(&digest) {
        return Err(AppError::BadRequest("digest must be a lowercase sha256:... (64 hex chars)".to_string()));
    }

    // Runtime images are treated as stock PostgreSQL deployments.
    // `pg_durable` is a built-in special image/mode and is not supported for arbitrary uploaded images.
    let requested_mode = req.suggested_image_type.to_lowercase();
    if requested_mode != "stock" {
        return Err(AppError::BadRequest(
            "Runtime images must use suggested_image_type='stock' (pg_durable is built-in only)".to_string(),
        ));
    }
    let suggested_image_type = "stock".to_string();

    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    let id = Uuid::new_v4();

    let row = sqlx::query(
        r#"
        INSERT INTO toygres_cms.runtime_images
            (id, name, description, acr_ref, digest, suggested_image_type, state)
        VALUES
            ($1, $2, $3, $4, $5, $6::toygres_cms.image_type, 'ready')
        ON CONFLICT (name) WHERE state != 'deleted' DO UPDATE
        SET description = EXCLUDED.description,
            acr_ref = EXCLUDED.acr_ref,
            digest = EXCLUDED.digest,
            suggested_image_type = EXCLUDED.suggested_image_type,
            updated_at = NOW()
        RETURNING id
        "#
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.acr_ref)
    .bind(&digest)
    .bind(&suggested_image_type)
    .fetch_one(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to register runtime image: {}", e)))?;

    let image_id: uuid::Uuid = {
        use sqlx::Row;
        row.get("id")
    };

    Ok(Json(serde_json::json!({
        "id": image_id.to_string(),
        "name": req.name,
        "acr_ref": req.acr_ref,
        "digest": digest,
        "suggested_image_type": suggested_image_type,
        "message": "Runtime image registered",
    })))
}

async fn bulk_create_instances(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    use uuid::Uuid;
    use toygres_orchestrations::types::CreateInstanceInput;
    use toygres_orchestrations::activity_types::ImageType;
    
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
    
    let image_type_str = req.get("image_type")
        .and_then(|v| v.as_str())
        .unwrap_or("stock");

    let runtime_image_id_str = req.get("runtime_image_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // If a runtime image is specified, resolve it to a canonical pull ref.
    // Runtime images are always deployed in stock mode (pg_durable is built-in only).
    let (runtime_image_id, image_override, image_type) = if let Some(runtime_image_id) = &runtime_image_id_str {
        let image_uuid = Uuid::parse_str(runtime_image_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid runtime_image_id: {}", e)))?;

        let pool = get_cms_pool().await
            .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

        let row = sqlx::query(
            r#"
            SELECT acr_ref, digest
            FROM toygres_cms.runtime_images
            WHERE id = $1 AND state != 'deleted'
            "#
        )
        .bind(image_uuid)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to resolve runtime image: {}", e)))?;

        let (acr_ref, digest): (String, String) = match row {
            Some(row) => {
                use sqlx::Row;
                (row.get("acr_ref"), row.get("digest"))
            }
            None => {
                return Err(AppError::BadRequest("Runtime image not found (or deleted)".to_string()));
            }
        };

        let pull_ref = format!("{}@{}", acr_ref, digest);
        (Some(runtime_image_id.clone()), Some(pull_ref), ImageType::Stock)
    } else {
        (None, None, ImageType::from_str(image_type_str))
    };
    
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
            image_type: image_type.clone(),
            source_image_id: None,
            runtime_image_id: runtime_image_id.clone(),
            image_override: image_override.clone(),
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
            "image_type": image_type.as_str(),
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
// Instance Lifecycle (Stop/Start/Restart)
// ============================================================================

async fn stop_instance(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::k8s_client::get_k8s_client;
    use k8s_openapi::api::apps::v1::StatefulSet;
    use kube::api::{Api, Patch, PatchParams};
    
    // Look up the instance by name
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT k8s_name, namespace, state::text FROM toygres_cms.instances WHERE dns_name = $1 AND state NOT IN ('deleted', 'deleting') LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, namespace, state) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    if state == "stopped" {
        return Ok(Json(serde_json::json!({
            "instance_name": name,
            "k8s_name": k8s_name,
            "status": "already_stopped",
            "message": "Instance is already stopped"
        })));
    }
    
    // Scale StatefulSet to 0
    let client = get_k8s_client().await
        .map_err(|e| AppError::Internal(format!("Failed to create K8s client: {}", e)))?;
    
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);
    
    let patch = serde_json::json!({
        "spec": {
            "replicas": 0
        }
    });
    
    statefulsets
        .patch(&k8s_name, &PatchParams::apply("toygres"), &Patch::Merge(&patch))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to stop instance: {}", e)))?;
    
    // Update CMS state to 'stopped'
    sqlx::query(
        "UPDATE toygres_cms.instances SET state = 'stopped', stopped_at = NOW(), updated_at = NOW() WHERE k8s_name = $1"
    )
    .bind(&k8s_name)
    .execute(&pool)
    .await
    .context("Failed to update instance state")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "status": "stopped",
        "message": "Instance stopped successfully"
    })))
}

async fn start_instance(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::k8s_client::get_k8s_client;
    use k8s_openapi::api::apps::v1::StatefulSet;
    use kube::api::{Api, Patch, PatchParams};
    
    // Look up the instance by name
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT k8s_name, namespace, state::text FROM toygres_cms.instances WHERE dns_name = $1 AND state NOT IN ('deleted', 'deleting') LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, namespace, state) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    if state == "running" {
        return Ok(Json(serde_json::json!({
            "instance_name": name,
            "k8s_name": k8s_name,
            "status": "already_running",
            "message": "Instance is already running"
        })));
    }
    
    // Scale StatefulSet to 1
    let client = get_k8s_client().await
        .map_err(|e| AppError::Internal(format!("Failed to create K8s client: {}", e)))?;
    
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);
    
    let patch = serde_json::json!({
        "spec": {
            "replicas": 1
        }
    });
    
    statefulsets
        .patch(&k8s_name, &PatchParams::apply("toygres"), &Patch::Merge(&patch))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start instance: {}", e)))?;
    
    // Update CMS state to 'running'
    sqlx::query(
        "UPDATE toygres_cms.instances SET state = 'running', started_at = NOW(), updated_at = NOW() WHERE k8s_name = $1"
    )
    .bind(&k8s_name)
    .execute(&pool)
    .await
    .context("Failed to update instance state")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "status": "started",
        "message": "Instance started successfully"
    })))
}

async fn restart_instance(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::k8s_client::get_k8s_client;
    use k8s_openapi::api::apps::v1::StatefulSet;
    use kube::api::{Api, Patch, PatchParams};
    
    // Look up the instance by name
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT k8s_name, namespace, state::text FROM toygres_cms.instances WHERE dns_name = $1 AND state NOT IN ('deleted', 'deleting') LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, namespace, state) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    if state == "stopped" {
        return Err(AppError::BadRequest("Cannot restart a stopped instance. Start it first.".to_string()));
    }
    
    // Trigger rollout restart by updating annotation
    let client = get_k8s_client().await
        .map_err(|e| AppError::Internal(format!("Failed to create K8s client: {}", e)))?;
    
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);
    
    let restart_time = chrono::Utc::now().to_rfc3339();
    let patch = serde_json::json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        "toygres.io/restartedAt": restart_time
                    }
                }
            }
        }
    });
    
    statefulsets
        .patch(&k8s_name, &PatchParams::apply("toygres"), &Patch::Merge(&patch))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to restart instance: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "status": "restarting",
        "restarted_at": restart_time,
        "message": "Instance restart triggered successfully"
    })))
}

// ============================================================================
// Instance Actor Control (Health Monitoring Orchestration)
// ============================================================================

/// Start the instance actor (health monitoring) orchestration for an instance
async fn start_instance_actor(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::types::InstanceActorInput;
    
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT k8s_name, namespace, state::text, instance_actor_orchestration_id 
         FROM toygres_cms.instances WHERE dns_name = $1 AND state NOT IN ('deleted', 'deleting') LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, namespace, _state, existing_actor_id) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    let actor_id = format!("actor-{}", k8s_name);
    
    // Check if actor already exists and is running
    if let Some(existing_id) = &existing_actor_id {
        if let Ok(info) = state.duroxide_client.get_instance_info(existing_id).await {
            if info.status == "Running" {
                return Err(AppError::BadRequest(format!(
                    "Instance actor '{}' is already running. Use restart to restart it.", existing_id
                )));
            }
        }
    }
    
    let actor_input = InstanceActorInput {
        k8s_name: k8s_name.clone(),
        namespace: namespace.clone(),
        orchestration_id: actor_id.clone(),
    };
    
    // Start the actor orchestration
    state.duroxide_client
        .start_orchestration(
            &actor_id,
            toygres_orchestrations::names::orchestrations::INSTANCE_ACTOR,
            &serde_json::to_string(&actor_input).unwrap(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start instance actor: {}", e)))?;
    
    // Update CMS with new actor ID
    sqlx::query(
        "UPDATE toygres_cms.instances SET instance_actor_orchestration_id = $1, updated_at = NOW() 
         WHERE k8s_name = $2"
    )
    .bind(&actor_id)
    .bind(&k8s_name)
    .execute(&pool)
    .await
    .context("Failed to update instance actor ID")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "actor_id": actor_id,
        "status": "started",
        "message": "Instance actor started successfully"
    })))
}

/// Restart the instance actor - cancels existing one and starts a new one
async fn restart_instance_actor(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use toygres_orchestrations::types::InstanceActorInput;
    
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT k8s_name, namespace, state::text, instance_actor_orchestration_id 
         FROM toygres_cms.instances WHERE dns_name = $1 AND state NOT IN ('deleted', 'deleting') LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, namespace, _state, existing_actor_id) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    // Cancel existing actor if it exists
    let mut cancelled_existing = false;
    if let Some(existing_id) = &existing_actor_id {
        if let Ok(info) = state.duroxide_client.get_instance_info(existing_id).await {
            if info.status == "Running" {
                // Cancel the existing actor gracefully
                if state.duroxide_client.cancel_instance(existing_id, "Restarting actor").await.is_ok() {
                    cancelled_existing = true;
                }
            }
        }
    }
    
    let actor_id = format!("actor-{}", k8s_name);
    
    let actor_input = InstanceActorInput {
        k8s_name: k8s_name.clone(),
        namespace: namespace.clone(),
        orchestration_id: actor_id.clone(),
    };
    
    // Start the actor orchestration
    state.duroxide_client
        .start_orchestration(
            &actor_id,
            toygres_orchestrations::names::orchestrations::INSTANCE_ACTOR,
            &serde_json::to_string(&actor_input).unwrap(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start instance actor: {}", e)))?;
    
    // Update CMS with new actor ID
    sqlx::query(
        "UPDATE toygres_cms.instances SET instance_actor_orchestration_id = $1, updated_at = NOW() 
         WHERE k8s_name = $2"
    )
    .bind(&actor_id)
    .bind(&k8s_name)
    .execute(&pool)
    .await
    .context("Failed to update instance actor ID")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "actor_id": actor_id,
        "cancelled_existing": cancelled_existing,
        "status": "restarted",
        "message": "Instance actor restarted successfully"
    })))
}

/// Cancel/stop the instance actor orchestration
async fn cancel_instance_actor(
    State(state): State<AppState>,
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
    
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT k8s_name, instance_actor_orchestration_id 
         FROM toygres_cms.instances WHERE dns_name = $1 AND state NOT IN ('deleted', 'deleting') LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (k8s_name, actor_id) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    let actor_id = match actor_id {
        Some(id) => id,
        None => return Err(AppError::BadRequest("Instance has no actor orchestration".to_string())),
    };
    
    // Cancel the actor using duroxide's cancel_instance
    state.duroxide_client
        .cancel_instance(&actor_id, "Cancelled by user")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to cancel instance actor: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "actor_id": actor_id,
        "status": "cancelled",
        "message": "Instance actor cancelled successfully"
    })))
}

// ============================================================================
// Instance Logs (PostgreSQL Pod Logs)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct InstanceLogsQuery {
    #[serde(default = "default_instance_log_lines")]
    tail_lines: i64,
    #[serde(default)]
    #[allow(dead_code)] // Reserved for future streaming logs implementation
    follow: bool,
}

fn default_instance_log_lines() -> i64 {
    200
}

async fn get_instance_logs(
    State(_state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<InstanceLogsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use k8s_openapi::api::core::v1::Pod;
    use kube::{Api, api::LogParams};
    
    // Look up the instance by dns_name to get k8s_name and namespace
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
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    // Get Kubernetes client
    let client = kube::Client::try_default()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create K8s client: {}", e)))?;
    
    // Pod name is <k8s_name>-0 for StatefulSet
    let pod_name = format!("{}-0", k8s_name);
    
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    
    // Build log params
    let log_params = LogParams {
        container: Some("postgres".to_string()),
        tail_lines: Some(query.tail_lines),
        timestamps: true,
        ..Default::default()
    };
    
    // Get logs
    let logs = pods
        .logs(&pod_name, &log_params)
        .await
        .map_err(|e| {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("not found") || error_msg.contains("NotFound") {
                AppError::NotFound(format!("Pod '{}' not found in namespace '{}'", pod_name, namespace))
            } else {
                AppError::Internal(format!("Failed to get logs: {}", e))
            }
        })?;
    
    // Split logs into lines
    let lines: Vec<&str> = logs.lines().collect();
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "k8s_name": k8s_name,
        "pod_name": pod_name,
        "namespace": namespace,
        "tail_lines": query.tail_lines,
        "log_count": lines.len(),
        "logs": lines,
    })))
}

// ============================================================================
// pg_durable Orchestrations (Durable SQL Functions)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct PgDurableQuery {
    #[serde(default = "default_pg_durable_limit")]
    limit: i64,
    #[serde(default)]
    status: Option<String>,
}

fn default_pg_durable_limit() -> i64 {
    50
}

/// Get pg_durable orchestration executions from a PostgreSQL instance
/// This connects to the instance's PostgreSQL and queries the df schema
async fn get_pg_durable_orchestrations(
    State(_state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<PgDurableQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    // First, look up the instance to get its connection string and verify it's a pg_durable instance
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT image_type::text, dns_connection_string, state::text
         FROM toygres_cms.instances 
         WHERE dns_name = $1 AND state != 'deleted'
         LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (image_type, connection_string, state) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    // Verify this is a pg_durable instance
    if image_type != "pg_durable" {
        return Err(AppError::BadRequest(format!(
            "Instance '{}' is not a pg_durable instance (image_type: {}). Durable orchestrations are only available for pg_durable instances.",
            name, image_type
        )));
    }
    
    // Verify instance is running
    if state != "running" {
        return Err(AppError::BadRequest(format!(
            "Instance '{}' is not running (state: {}). Cannot query durable orchestrations.",
            name, state
        )));
    }
    
    let connection_string = connection_string.ok_or_else(|| {
        AppError::Internal("Instance has no connection string".to_string())
    })?;
    
    // Connect to the pg_durable instance
    let instance_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&connection_string)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to connect to pg_durable instance: {}", e)))?;
    
    // Query the pg_durable instances using df.list_instances() function
    let status_filter = query.status.as_deref();
    
    let functions = sqlx::query_as::<_, (String, Option<String>, Option<String>, String, i64, Option<String>)>(
        r#"
        SELECT 
            instance_id,
            label,
            function_name,
            status,
            execution_count,
            output
        FROM df.list_instances($1::text, $2::integer)
        "#
    )
    .bind(status_filter)
    .bind(query.limit as i32)
    .fetch_all(&instance_pool)
    .await
    .map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("does not exist") {
            AppError::BadRequest(format!("df schema error: {}", err_str))
        } else {
            AppError::Internal(format!("Failed to query pg_durable instances: {}", e))
        }
    })?;
    
    let functions_json: Vec<serde_json::Value> = functions
        .into_iter()
        .map(|(instance_id, label, function_name, status, execution_count, output)| {
            serde_json::json!({
                "instance_id": instance_id,
                "label": label,
                "function_name": function_name,
                "status": status,
                "execution_count": execution_count,
                "output": output,
            })
        })
        .collect();
    
    Ok(Json(serde_json::json!({
        "instance_name": name,
        "image_type": image_type,
        "count": functions_json.len(),
        "functions": functions_json,
    })))
}

#[derive(Debug, serde::Deserialize)]
struct InstanceNodesQuery {
    #[serde(default = "default_executions")]
    executions: i32,
}

fn default_executions() -> i32 {
    5
}

/// Get nodes for a specific pg_durable orchestration instance
/// Calls df.instance_nodes(instance_id, last_n_executions) function
async fn get_pg_durable_instance_nodes(
    State(_state): State<AppState>,
    Path((name, instance_id)): Path<(String, String)>,
    Query(query): Query<InstanceNodesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    // First, look up the instance to get its connection string
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT image_type::text, dns_connection_string, state::text
         FROM toygres_cms.instances 
         WHERE dns_name = $1 AND state != 'deleted'
         LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (image_type, connection_string, state) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    if image_type != "pg_durable" {
        return Err(AppError::BadRequest("Not a pg_durable instance".to_string()));
    }
    
    if state != "running" {
        return Err(AppError::BadRequest(format!("Instance is not running (state: {})", state)));
    }
    
    let connection_string = connection_string.ok_or_else(|| {
        AppError::Internal("Instance has no connection string".to_string())
    })?;
    
    // Connect to the pg_durable instance
    let instance_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&connection_string)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to connect to pg_durable instance: {}", e)))?;
    
    // Call df.instance_nodes(instance_id, last_n_executions)
    let nodes = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, Option<String>)>(
        "SELECT execution_id, node_id, node_type, query, result_name, left_node, right_node, status, result 
         FROM df.instance_nodes($1, $2)"
    )
    .bind(&instance_id)
    .bind(query.executions)
    .fetch_all(&instance_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to query instance nodes: {}", e)))?;
    
    let nodes_json: Vec<serde_json::Value> = nodes
        .into_iter()
        .map(|(execution_id, node_id, node_type, query, result_name, left_node, right_node, status, result)| {
            serde_json::json!({
                "execution_id": execution_id,
                "node_id": node_id,
                "node_type": node_type,
                "query": query,
                "result_name": result_name,
                "left_node": left_node,
                "right_node": right_node,
                "status": status,
                "result": result,
            })
        })
        .collect();
    
    Ok(Json(serde_json::json!({
        "pg_instance_name": name,
        "orchestration_instance_id": instance_id,
        "executions_shown": query.executions,
        "count": nodes_json.len(),
        "nodes": nodes_json,
    })))
}

/// Get explain output for a specific pg_durable orchestration instance
/// Calls df.explain(instance_id) function for visualization
async fn get_pg_durable_explain(
    State(_state): State<AppState>,
    Path((name, instance_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    
    // First, look up the instance to get its connection string
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| AppError::Internal("DATABASE_URL not configured".to_string()))?;
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let row = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT image_type::text, dns_connection_string, state::text
         FROM toygres_cms.instances 
         WHERE dns_name = $1 AND state != 'deleted'
         LIMIT 1"
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query instance")
    .map_err(|e| AppError::Internal(e.to_string()))?;
    
    let (image_type, connection_string, state) = match row {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Instance '{}' not found", name))),
    };
    
    if image_type != "pg_durable" {
        return Err(AppError::BadRequest("Not a pg_durable instance".to_string()));
    }
    
    if state != "running" {
        return Err(AppError::BadRequest(format!("Instance is not running (state: {})", state)));
    }
    
    let connection_string = connection_string.ok_or_else(|| {
        AppError::Internal("Instance has no connection string".to_string())
    })?;
    
    // Connect to the pg_durable instance
    let instance_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&connection_string)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to connect to pg_durable instance: {}", e)))?;
    
    // Call df.explain(instance_id)
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT df.explain($1)"
    )
    .bind(&instance_id)
    .fetch_one(&instance_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to get explain output: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "pg_instance_name": name,
        "orchestration_instance_id": instance_id,
        "explain": result.0,
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
    for instance_id in instance_ids.iter() {
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
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Use cancel_instance to cancel any orchestration by its instance ID
    state.duroxide_client
        .cancel_instance(&id, "Cancelled by user via management API")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to cancel orchestration: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "instance_id": id,
        "status": "cancelled",
        "message": "Orchestration cancellation requested"
    })))
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
// Orchestration Instance Management (Delete, Prune, Tree)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct DeleteOrchestrationQuery {
    #[serde(default)]
    force: bool,
}

async fn delete_orchestration_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<DeleteOrchestrationQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !state.duroxide_client.has_management_capability() {
        return Err(AppError::Internal("Management features not available".to_string()));
    }

    // Get instance info first for logging
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

    // Check if running and force is not set
    if info.status == "Running" && !params.force {
        return Err(AppError::BadRequest(
            "Cannot delete running orchestration. Use force=true to force delete.".to_string()
        ));
    }

    // Delete the instance
    state.duroxide_client
        .delete_instance(&id, params.force)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to delete instance: {}", e)))?;

    tracing::info!(
        instance_id = %id,
        orchestration_name = %info.orchestration_name,
        status = %info.status,
        force = params.force,
        "Deleted orchestration instance"
    );

    Ok(Json(serde_json::json!({
        "instance_id": id,
        "orchestration_name": info.orchestration_name,
        "status": info.status,
        "deleted": true,
        "force": params.force,
    })))
}

#[derive(Debug, serde::Deserialize)]
struct PruneOrchestrationRequest {
    /// Keep only the last N executions (default: 1, meaning keep only current)
    #[serde(default = "default_keep_executions")]
    keep_executions: u32,
}

fn default_keep_executions() -> u32 {
    1
}

async fn prune_orchestration(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PruneOrchestrationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !state.duroxide_client.has_management_capability() {
        return Err(AppError::Internal("Management features not available".to_string()));
    }

    // Get current execution count before pruning
    let executions_before = state.duroxide_client
        .list_executions(&id)
        .await
        .map_err(|e| {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("not found") || error_msg.contains("NotFound") {
                AppError::NotFound(format!("Orchestration '{}' not found", id))
            } else {
                AppError::Internal(format!("Failed to list executions: {}", e))
            }
        })?;

    let count_before = executions_before.len();

    // Prune executions
    // keep_last: None means prune all except current execution
    // keep_last: Some(N) keeps the top N executions
    let prune_options = duroxide::PruneOptions {
        keep_last: if req.keep_executions == 0 {
            None
        } else {
            Some(req.keep_executions)
        },
        completed_before: None,
    };

    state.duroxide_client
        .prune_executions(&id, prune_options)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to prune executions: {}", e)))?;

    // Get execution count after pruning
    let executions_after = state.duroxide_client
        .list_executions(&id)
        .await
        .unwrap_or_default();

    let count_after = executions_after.len();
    let pruned_count = count_before.saturating_sub(count_after);

    tracing::info!(
        instance_id = %id,
        executions_before = count_before,
        executions_after = count_after,
        pruned = pruned_count,
        keep_executions = req.keep_executions,
        "Pruned orchestration executions"
    );

    Ok(Json(serde_json::json!({
        "instance_id": id,
        "executions_before": count_before,
        "executions_after": count_after,
        "pruned": pruned_count,
        "keep_executions": req.keep_executions,
    })))
}

async fn get_orchestration_tree(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !state.duroxide_client.has_management_capability() {
        return Err(AppError::Internal("Management features not available".to_string()));
    }

    // Get instance info
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

    // Get the instance tree (includes all descendants)
    let tree = state.duroxide_client
        .get_instance_tree(&id)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get instance tree: {}", e)))?;

    // Get parent info if this instance has a parent
    let parent_info = if let Some(ref parent_id) = info.parent_instance_id {
        state.duroxide_client
            .get_instance_info(parent_id)
            .await
            .ok()
            .map(|p| serde_json::json!({
                "instance_id": p.instance_id,
                "orchestration_name": p.orchestration_name,
                "status": p.status,
            }))
    } else {
        None
    };

    // Get info for all descendants (excluding self)
    let mut children = Vec::new();
    for child_id in &tree.all_ids {
        if child_id == &id {
            continue; // Skip self
        }
        if let Ok(child_info) = state.duroxide_client.get_instance_info(child_id).await {
            let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(child_info.created_at as i64)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string());

            children.push(serde_json::json!({
                "instance_id": child_info.instance_id,
                "orchestration_name": child_info.orchestration_name,
                "status": child_info.status,
                "created_at": created_at,
                "is_direct_child": child_info.parent_instance_id.as_ref() == Some(&id),
            }));
        }
    }

    // Get execution count for context
    let execution_count = state.duroxide_client
        .list_executions(&id)
        .await
        .map(|e| e.len())
        .unwrap_or(0);

    let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(info.created_at as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(Json(serde_json::json!({
        "instance_id": info.instance_id,
        "orchestration_name": info.orchestration_name,
        "status": info.status,
        "created_at": created_at,
        "execution_count": execution_count,
        "parent": parent_info,
        "children": children,
        "children_count": children.len(),
        "tree_size": tree.all_ids.len(),
        "is_root": tree.root_id == id,
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
    // Check if running in Kubernetes
    if std::env::var("KUBERNETES_DEPLOYMENT").is_ok() {
        return get_logs_from_kubernetes(query).await;
    }
    
    // Local development: read from log file
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

/// Get logs from Kubernetes pod (when running in K8s)
async fn get_logs_from_kubernetes(query: LogsQuery) -> Result<Json<Vec<String>>, AppError> {
    use k8s_openapi::api::core::v1::Pod;
    use kube::{api::Api, Client};
    
    let client = Client::try_default()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create K8s client: {}", e)))?;
    
    // Get our own pod name from environment (set by Kubernetes)
    let pod_name = std::env::var("HOSTNAME")
        .unwrap_or_else(|_| "toygres-server".to_string());
    
    let namespace = std::env::var("POD_NAMESPACE")
        .unwrap_or_else(|_| "toygres-system".to_string());
    
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    
    // Get logs with tail
    let log_params = kube::api::LogParams {
        tail_lines: Some(query.limit as i64),
        timestamps: true,
        ..Default::default()
    };
    
    let logs = pods.logs(&pod_name, &log_params)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get pod logs: {}", e)))?;
    
    let mut lines: Vec<String> = logs
        .lines()
        .map(|s| s.to_string())
        .collect();
    
    // Apply filter if provided
    if let Some(ref filter) = query.filter {
        let filter_lower = filter.to_lowercase();
        lines.retain(|line| line.to_lowercase().contains(&filter_lower));
    }
    
    Ok(Json(lines))
}

// ============================================================================
// System Pruner Log
// ============================================================================

/// Get the prune log from the system pruner orchestration
///
/// Returns the last prune activity results, showing what instances were deleted
/// and what executions were pruned.
async fn get_prune_log(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    const SYSTEM_PRUNER_ID: &str = "system-pruner";

    // Check if system pruner exists
    let info = state.duroxide_client
        .get_instance_info(SYSTEM_PRUNER_ID)
        .await
        .map_err(|_| AppError::NotFound("System pruner not running".to_string()))?;

    // Get all execution history to find the latest prune logs
    let execution_ids = state.duroxide_client
        .list_executions(SYSTEM_PRUNER_ID)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list executions: {}", e)))?;

    let mut prune_logs: Vec<serde_json::Value> = Vec::new();
    let mut last_run: Option<String> = None;
    let mut iteration: u64 = 0;

    // Get events from the most recent execution(s) to find prune activity results
    // Process from newest to oldest, but only look at the last 10 executions
    for exec_id in execution_ids.iter().rev().take(10) {
        if let Ok(events) = state.duroxide_client.read_execution_history(SYSTEM_PRUNER_ID, *exec_id).await {
            for event in events {
                // Extract last run time from orchestration start event
                if matches!(event.kind, EventKind::OrchestrationStarted { .. }) {
                    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(event.timestamp_ms as i64)
                        .map(|dt| dt.to_rfc3339());
                    if last_run.is_none() {
                        last_run = dt;
                    }
                }

                // Extract prune logs from activity completion events
                if let EventKind::ActivityCompleted { result } = &event.kind {
                    // The result is a JSON string containing SystemPruneOutput
                    if let Ok(output) = serde_json::from_str::<serde_json::Value>(result) {
                        // Extract iteration number
                        if let Some(iter) = output.get("iteration").and_then(|v| v.as_u64()) {
                            if iter > iteration {
                                iteration = iter;
                            }
                        }

                        // Extract prune_log array
                        if let Some(logs) = output.get("prune_log").and_then(|v| v.as_array()) {
                            for log in logs {
                                prune_logs.push(log.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort logs by timestamp (newest first)
    prune_logs.sort_by(|a, b| {
        let ts_a = a.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let ts_b = b.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        ts_b.cmp(ts_a)
    });

    // Limit to last 50 entries
    prune_logs.truncate(50);

    let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(info.created_at as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(Json(serde_json::json!({
        "instance_id": SYSTEM_PRUNER_ID,
        "status": info.status,
        "orchestration_version": info.orchestration_version,
        "current_execution_id": info.current_execution_id,
        "created_at": created_at,
        "last_run": last_run,
        "iteration": iteration,
        "prune_log": prune_logs,
        "total_entries": prune_logs.len(),
    })))
}

// ============================================================================
// Image API Handlers
// ============================================================================

#[derive(serde::Deserialize)]
struct CreateImageRequest {
    name: String,
    description: Option<String>,
    /// Password is optional - if not provided, it will be fetched from K8s Secret
    #[serde(default)]
    password: Option<String>,
}

async fn list_images(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Get database pool directly since we need to query CMS
    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;
    
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, source_instance_id, source_k8s_name, source_namespace,
               blob_storage_url, blob_container, blob_path,
               storage_size_gb, postgres_version, image_type::text,
               backup_size_bytes, backup_checksum, state::text,
               error_message, created_at, ready_at
        FROM toygres_cms.images
        WHERE state != 'deleted'
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to list images: {}", e)))?;

    let images: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            use chrono::{DateTime, Utc};
            
            let created_at: DateTime<Utc> = row.get("created_at");
            let ready_at: Option<DateTime<Utc>> = row.get("ready_at");
            
            serde_json::json!({
                "id": row.get::<uuid::Uuid, _>("id").to_string(),
                "name": row.get::<String, _>("name"),
                "description": row.get::<Option<String>, _>("description"),
                "source_k8s_name": row.get::<String, _>("source_k8s_name"),
                "state": row.get::<String, _>("state"),
                "storage_size_gb": row.get::<i32, _>("storage_size_gb"),
                "postgres_version": row.get::<String, _>("postgres_version"),
                "image_type": row.get::<String, _>("image_type"),
                "backup_size_bytes": row.get::<Option<i64>, _>("backup_size_bytes"),
                "created_at": created_at.to_rfc3339(),
                "ready_at": ready_at.map(|t| t.to_rfc3339()),
            })
        })
        .collect();
    
    Ok(Json(serde_json::json!(images)))
}

async fn get_image(
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;
    
    let row = sqlx::query(
        r#"
        SELECT id, name, description, source_instance_id, source_k8s_name, source_namespace,
               blob_storage_url, blob_container, blob_path,
               storage_size_gb, postgres_version, image_type::text,
               backup_size_bytes, backup_checksum, state::text,
               error_message, created_at, ready_at, create_orchestration_id
        FROM toygres_cms.images
        WHERE name = $1 AND state != 'deleted'
        "#
    )
    .bind(&name)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to get image: {}", e)))?;

    match row {
        Some(row) => {
            use sqlx::Row;
            use chrono::{DateTime, Utc};
            
            let created_at: DateTime<Utc> = row.get("created_at");
            let ready_at: Option<DateTime<Utc>> = row.get("ready_at");
            
            Ok(Json(serde_json::json!({
                "id": row.get::<uuid::Uuid, _>("id").to_string(),
                "name": row.get::<String, _>("name"),
                "description": row.get::<Option<String>, _>("description"),
                "source_instance_id": row.get::<Option<uuid::Uuid>, _>("source_instance_id").map(|u| u.to_string()),
                "source_k8s_name": row.get::<String, _>("source_k8s_name"),
                "source_namespace": row.get::<String, _>("source_namespace"),
                "blob_storage_url": row.get::<String, _>("blob_storage_url"),
                "blob_container": row.get::<String, _>("blob_container"),
                "blob_path": row.get::<String, _>("blob_path"),
                "state": row.get::<String, _>("state"),
                "storage_size_gb": row.get::<i32, _>("storage_size_gb"),
                "postgres_version": row.get::<String, _>("postgres_version"),
                "image_type": row.get::<String, _>("image_type"),
                "backup_size_bytes": row.get::<Option<i64>, _>("backup_size_bytes"),
                "backup_checksum": row.get::<Option<String>, _>("backup_checksum"),
                "error_message": row.get::<Option<String>, _>("error_message"),
                "created_at": created_at.to_rfc3339(),
                "ready_at": ready_at.map(|t| t.to_rfc3339()),
                "orchestration_id": row.get::<String, _>("create_orchestration_id"),
            })))
        }
        None => Err(AppError::NotFound(format!("Image '{}' not found", name))),
    }
}

/// Get backup job logs for an image
async fn get_image_job_logs(
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    use kube::{Api, Client};
    use k8s_openapi::api::batch::v1::Job;
    use k8s_openapi::api::core::v1::Pod;
    
    // Get the orchestration_id from CMS to construct the job name
    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;
    
    let row = sqlx::query(
        "SELECT create_orchestration_id, source_namespace FROM toygres_cms.images WHERE name = $1"
    )
    .bind(&name)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to get image: {}", e)))?;
    
    let (orchestration_id, namespace): (String, String) = match row {
        Some(row) => {
            use sqlx::Row;
            (row.get("create_orchestration_id"), row.get("source_namespace"))
        }
        None => return Err(AppError::NotFound(format!("Image '{}' not found", name))),
    };
    
    // Job name format: backup-{sanitized_name}-{orchestration_id_prefix}
    // The orchestration_id is like "create-image-{name}-{uuid_prefix}"
    // The job name uses the first 8 chars of the orchestration_id (the uuid part)
    let sanitized_name = name.to_lowercase().replace('_', "-");
    let orch_id_suffix = orchestration_id.split('-').last().unwrap_or("unknown");
    let job_name = format!("backup-{}-{}", sanitized_name, orch_id_suffix);
    
    // Get K8s client
    let client = Client::try_default()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create K8s client: {}", e)))?;
    
    // Try to get job logs from the pod
    let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
    
    // Find pods for the job
    let pod_list = pods.list(&kube::api::ListParams::default().labels(&format!("job-name={}", job_name)))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list pods: {}", e)))?;
    
    let mut logs = String::new();
    let mut job_status = "unknown";
    
    if let Some(pod) = pod_list.items.first() {
        let pod_name = pod.metadata.name.as_deref().unwrap_or("unknown");
        
        // Get pod phase
        if let Some(status) = &pod.status {
            job_status = match status.phase.as_deref() {
                Some("Succeeded") => "completed",
                Some("Failed") => "failed",
                Some("Running") => "running",
                Some("Pending") => "pending",
                _ => "unknown",
            };
        }
        
        // Get logs
        match pods.logs(pod_name, &kube::api::LogParams {
            tail_lines: Some(100),
            ..Default::default()
        }).await {
            Ok(pod_logs) => logs = pod_logs,
            Err(e) => logs = format!("Failed to get logs: {}", e),
        }
    } else {
        // Check if job exists but pod is gone
        let jobs: Api<Job> = Api::namespaced(client, &namespace);
        match jobs.get(&job_name).await {
            Ok(job) => {
                if let Some(status) = job.status {
                    if status.succeeded.unwrap_or(0) > 0 {
                        job_status = "completed";
                        logs = "Job completed. Pod logs no longer available (pod cleaned up).".to_string();
                    } else if status.failed.unwrap_or(0) > 0 {
                        job_status = "failed";
                        logs = "Job failed. Pod logs no longer available (pod cleaned up).".to_string();
                    }
                }
            }
            Err(kube::Error::Api(e)) if e.code == 404 => {
                // Job doesn't exist anymore - it was cleaned up
                job_status = "cleaned_up";
                logs = "Backup job has been cleaned up. Logs are no longer available.".to_string();
            }
            Err(e) => {
                return Err(AppError::Internal(format!("Failed to get job: {}", e)));
            }
        }
    }
    
    Ok(Json(serde_json::json!({
        "image_name": name,
        "job_name": job_name,
        "job_status": job_status,
        "logs": logs,
    })))
}

async fn create_image(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    use toygres_orchestrations::types::CreateImageInput;
    
    let name = req.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing image name".to_string()))?;
    
    let source_k8s_name = req.get("source_k8s_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing source_k8s_name".to_string()))?;
    
    // Password is optional - if not provided, orchestration will fetch from K8s Secret
    let password = req.get("password")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let description = req.get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let namespace = req.get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("toygres");
    
    // Validate name
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest("Invalid image name. Use only alphanumeric characters, hyphens, and underscores.".to_string()));
    }
    
    let orchestration_id = format!("create-image-{}-{}", name, uuid::Uuid::new_v4().to_string().split('-').next().unwrap());
    
    let input = CreateImageInput {
        name: name.to_string(),
        description,
        source_k8s_name: source_k8s_name.to_string(),
        source_password: password,
        namespace: Some(namespace.to_string()),
        orchestration_id: orchestration_id.clone(),
    };
    
    state.duroxide_client
        .start_orchestration(
            &orchestration_id,
            toygres_orchestrations::names::orchestrations::CREATE_IMAGE,
            &serde_json::to_string(&input).unwrap(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start orchestration: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "image_name": name,
        "orchestration_id": orchestration_id,
        "message": "Image creation started"
    })))
}

async fn create_image_from_instance(
    State(state): State<AppState>,
    Path(instance_name): Path<String>,
    Json(req): Json<CreateImageRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use toygres_orchestrations::types::CreateImageInput;
    
    // Validate image name
    if req.name.is_empty() || !req.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest("Invalid image name. Use only alphanumeric characters, hyphens, and underscores.".to_string()));
    }
    
    // Look up instance by user_name to get k8s_name
    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;
    
    let row = sqlx::query(
        "SELECT k8s_name, namespace FROM toygres_cms.instances WHERE user_name = $1 AND state = 'running'"
    )
    .bind(&instance_name)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to query instance: {}", e)))?;
    
    let (k8s_name, namespace): (String, String) = match row {
        Some(row) => {
            use sqlx::Row;
            (row.get("k8s_name"), row.get("namespace"))
        }
        None => {
            return Err(AppError::NotFound(format!("Instance '{}' not found or not running", instance_name)));
        }
    };
    
    let orchestration_id = format!("create-image-{}-{}", req.name, uuid::Uuid::new_v4().to_string().split('-').next().unwrap());
    
    let input = CreateImageInput {
        name: req.name.clone(),
        description: req.description,
        source_k8s_name: k8s_name,
        source_password: req.password,
        namespace: Some(namespace),
        orchestration_id: orchestration_id.clone(),
    };
    
    state.duroxide_client
        .start_orchestration(
            &orchestration_id,
            toygres_orchestrations::names::orchestrations::CREATE_IMAGE,
            &serde_json::to_string(&input).unwrap(),
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start orchestration: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "image_name": req.name,
        "source_instance": instance_name,
        "orchestration_id": orchestration_id,
        "message": "Image creation started"
    })))
}

async fn delete_image(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // For now, just mark as deleted in CMS
    // TODO: Implement DeleteImageOrchestration to also delete blob storage
    let pool = get_cms_pool().await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;
    
    let result = sqlx::query(
        r#"
        UPDATE toygres_cms.images
        SET state = 'deleted'::toygres_cms.image_state,
            deleted_at = NOW(),
            updated_at = NOW()
        WHERE name = $1 AND state != 'deleted'
        "#
    )
    .bind(&name)
    .execute(&*pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to delete image: {}", e)))?;
    
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Image '{}' not found", name)));
    }
    
    Ok(Json(serde_json::json!({
        "image_name": name,
        "deleted": true,
        "message": "Image deleted (blob storage cleanup pending)"
    })))
}

async fn get_cms_pool() -> Result<std::sync::Arc<sqlx::PgPool>, String> {
    use std::sync::OnceLock;
    use std::sync::Arc;
    
    static POOL: OnceLock<Arc<sqlx::PgPool>> = OnceLock::new();
    
    if let Some(pool) = POOL.get() {
        return Ok(pool.clone());
    }
    
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL not set".to_string())?;
    
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
    let arc_pool = Arc::new(pool);
    let _ = POOL.set(arc_pool.clone());
    
    Ok(arc_pool)
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
