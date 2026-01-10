//! Consolidated CMS activity for image operations
//! 
//! Uses an enum-based approach to handle all image CRUD operations
//! in a single activity, reducing boilerplate while maintaining type safety.

use duroxide::ActivityContext;
use sqlx::Row;
use uuid::Uuid;

use crate::activity_types::{ImageOperation, ImageOperationResult, ImageRecord, ImageState};
use super::get_pool;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::cms-image-ops";

pub async fn activity(
    ctx: ActivityContext,
    input: ImageOperation,
) -> Result<ImageOperationResult, String> {
    let pool = get_pool().await?;
    
    match input {
        ImageOperation::Create {
            name,
            description,
            source_instance_id,
            source_k8s_name,
            source_namespace,
            blob_storage_url,
            blob_container,
            blob_path,
            storage_size_gb,
            postgres_version,
            image_type,
            source_password_encrypted,
            orchestration_id,
        } => {
            ctx.trace_info(format!("Creating image record: {}", name));
            
            // Idempotent insert using orchestration_id for conflict resolution
            let result = sqlx::query(
                r#"
                INSERT INTO toygres_cms.images (
                    name, description, source_instance_id, source_k8s_name, source_namespace,
                    blob_storage_url, blob_container, blob_path,
                    storage_size_gb, postgres_version, image_type,
                    source_password_encrypted, state, create_orchestration_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::toygres_cms.image_type, $12, 'creating', $13)
                ON CONFLICT (name) WHERE state != 'deleted' DO UPDATE
                SET description = EXCLUDED.description,
                    updated_at = NOW()
                WHERE toygres_cms.images.create_orchestration_id = EXCLUDED.create_orchestration_id
                RETURNING id
                "#
            )
            .bind(&name)
            .bind(&description)
            .bind(source_instance_id)
            .bind(&source_k8s_name)
            .bind(&source_namespace)
            .bind(&blob_storage_url)
            .bind(&blob_container)
            .bind(&blob_path)
            .bind(storage_size_gb)
            .bind(&postgres_version)
            .bind(&image_type)
            .bind(&source_password_encrypted)
            .bind(&orchestration_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to create image record: {}", e))?;

            match result {
                Some(row) => {
                    let id: Uuid = row.try_get("id")
                        .map_err(|e| format!("Failed to get image id: {}", e))?;
                    ctx.trace_info(format!("Image record created: {}", id));
                    Ok(ImageOperationResult::Created { image_id: id })
                }
                None => {
                    // Conflict with different orchestration - fetch existing
                    let existing = sqlx::query(
                        "SELECT id FROM toygres_cms.images WHERE name = $1 AND state != 'deleted'"
                    )
                    .bind(&name)
                    .fetch_optional(&pool)
                    .await
                    .map_err(|e| format!("Failed to fetch existing image: {}", e))?;
                    
                    match existing {
                        Some(row) => {
                            let id: Uuid = row.try_get("id")
                                .map_err(|e| format!("Failed to get existing image id: {}", e))?;
                            ctx.trace_info(format!("Image already exists (replay): {}", id));
                            Ok(ImageOperationResult::Created { image_id: id })
                        }
                        None => Err("Image creation conflict but no existing record found".to_string())
                    }
                }
            }
        }
        
        ImageOperation::UpdateState {
            name,
            state,
            backup_size_bytes,
            backup_checksum,
            error_message,
        } => {
            ctx.trace_info(format!("Updating image state: {} -> {}", name, state.as_str()));
            
            let state_str = state.as_str();
            let ready_at_expr = if matches!(state, ImageState::Ready) {
                "NOW()"
            } else {
                "ready_at"
            };
            
            // Dynamic query to conditionally set ready_at
            let query = format!(
                r#"
                UPDATE toygres_cms.images
                SET state = $2::toygres_cms.image_state,
                    backup_size_bytes = COALESCE($3, backup_size_bytes),
                    backup_checksum = COALESCE($4, backup_checksum),
                    error_message = $5,
                    ready_at = {},
                    updated_at = NOW()
                WHERE name = $1 AND state != 'deleted'
                "#,
                ready_at_expr
            );
            
            let result = sqlx::query(&query)
                .bind(&name)
                .bind(state_str)
                .bind(backup_size_bytes)
                .bind(&backup_checksum)
                .bind(&error_message)
                .execute(&pool)
                .await
                .map_err(|e| format!("Failed to update image state: {}", e))?;

            let updated = result.rows_affected() > 0;
            ctx.trace_info(format!("Image state updated: {}", updated));
            Ok(ImageOperationResult::Updated { updated })
        }
        
        ImageOperation::Get { name } => {
            ctx.trace_info(format!("Getting image: {}", name));
            
            let row = sqlx::query(
                r#"
                SELECT id, name, description, source_instance_id, source_k8s_name, source_namespace,
                       blob_storage_url, blob_container, blob_path,
                       storage_size_gb, postgres_version, image_type::text,
                       backup_size_bytes, backup_checksum, state::text,
                       error_message, created_at, ready_at
                FROM toygres_cms.images
                WHERE name = $1 AND state != 'deleted'
                "#
            )
            .bind(&name)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to get image: {}", e))?;

            match row {
                Some(row) => {
                    let record = row_to_image_record(&row)?;
                    Ok(ImageOperationResult::Found { record })
                }
                None => Ok(ImageOperationResult::NotFound)
            }
        }
        
        ImageOperation::GetById { id } => {
            ctx.trace_info(format!("Getting image by ID: {}", id));
            
            let row = sqlx::query(
                r#"
                SELECT id, name, description, source_instance_id, source_k8s_name, source_namespace,
                       blob_storage_url, blob_container, blob_path,
                       storage_size_gb, postgres_version, image_type::text,
                       backup_size_bytes, backup_checksum, state::text,
                       error_message, created_at, ready_at
                FROM toygres_cms.images
                WHERE id = $1 AND state != 'deleted'
                "#
            )
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to get image by id: {}", e))?;

            match row {
                Some(row) => {
                    let record = row_to_image_record(&row)?;
                    Ok(ImageOperationResult::Found { record })
                }
                None => Ok(ImageOperationResult::NotFound)
            }
        }
        
        ImageOperation::List => {
            ctx.trace_info("Listing all active images");
            
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
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Failed to list images: {}", e))?;

            let images: Result<Vec<ImageRecord>, String> = rows
                .iter()
                .map(row_to_image_record)
                .collect();
            
            Ok(ImageOperationResult::Listed { images: images? })
        }
        
        ImageOperation::Delete { name } => {
            ctx.trace_info(format!("Deleting image: {}", name));

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
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete image: {}", e))?;

            let deleted = result.rows_affected() > 0;
            ctx.trace_info(format!("Image deleted: {}", deleted));
            Ok(ImageOperationResult::Deleted { deleted })
        }

        ImageOperation::GetSourcePassword { id } => {
            ctx.trace_info(format!("Getting source password for image: {}", id));

            let row = sqlx::query(
                r#"
                SELECT source_password_encrypted
                FROM toygres_cms.images
                WHERE id = $1 AND state != 'deleted'
                "#
            )
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to get source password: {}", e))?;

            match row {
                Some(row) => {
                    let encrypted: Vec<u8> = row.try_get("source_password_encrypted")
                        .map_err(|e| format!("Failed to get source_password_encrypted: {}", e))?;

                    // Decrypt password (currently just UTF-8 bytes, but could use proper encryption)
                    let password = String::from_utf8(encrypted)
                        .map_err(|e| format!("Failed to decrypt password: {}", e))?;

                    ctx.trace_info("Source password retrieved");
                    Ok(ImageOperationResult::PasswordFound { password })
                }
                None => {
                    ctx.trace_warn(format!("Image not found: {}", id));
                    Ok(ImageOperationResult::NotFound)
                }
            }
        }
    }
}

fn row_to_image_record(row: &sqlx::postgres::PgRow) -> Result<ImageRecord, String> {
    use chrono::{DateTime, Utc};
    
    let created_at: DateTime<Utc> = row.try_get("created_at")
        .map_err(|e| format!("Failed to get created_at: {}", e))?;
    let ready_at: Option<DateTime<Utc>> = row.try_get("ready_at")
        .map_err(|e| format!("Failed to get ready_at: {}", e))?;
    
    Ok(ImageRecord {
        id: row.try_get("id").map_err(|e| format!("Failed to get id: {}", e))?,
        name: row.try_get("name").map_err(|e| format!("Failed to get name: {}", e))?,
        description: row.try_get("description").map_err(|e| format!("Failed to get description: {}", e))?,
        source_instance_id: row.try_get("source_instance_id").map_err(|e| format!("Failed to get source_instance_id: {}", e))?,
        source_k8s_name: row.try_get("source_k8s_name").map_err(|e| format!("Failed to get source_k8s_name: {}", e))?,
        source_namespace: row.try_get("source_namespace").map_err(|e| format!("Failed to get source_namespace: {}", e))?,
        blob_storage_url: row.try_get("blob_storage_url").map_err(|e| format!("Failed to get blob_storage_url: {}", e))?,
        blob_container: row.try_get("blob_container").map_err(|e| format!("Failed to get blob_container: {}", e))?,
        blob_path: row.try_get("blob_path").map_err(|e| format!("Failed to get blob_path: {}", e))?,
        storage_size_gb: row.try_get("storage_size_gb").map_err(|e| format!("Failed to get storage_size_gb: {}", e))?,
        postgres_version: row.try_get("postgres_version").map_err(|e| format!("Failed to get postgres_version: {}", e))?,
        image_type: row.try_get("image_type").map_err(|e| format!("Failed to get image_type: {}", e))?,
        backup_size_bytes: row.try_get("backup_size_bytes").map_err(|e| format!("Failed to get backup_size_bytes: {}", e))?,
        backup_checksum: row.try_get("backup_checksum").map_err(|e| format!("Failed to get backup_checksum: {}", e))?,
        state: row.try_get("state").map_err(|e| format!("Failed to get state: {}", e))?,
        error_message: row.try_get("error_message").map_err(|e| format!("Failed to get error_message: {}", e))?,
        created_at: created_at.to_rfc3339(),
        ready_at: ready_at.map(|t| t.to_rfc3339()),
    })
}
