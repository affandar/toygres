use duroxide::ActivityContext;
use sqlx::{Error as SqlxError, Row};
use uuid::Uuid;

use crate::activity_types::{CreateInstanceRecordInput, CreateInstanceRecordOutput};

use super::get_pool;

pub async fn create_instance_record_activity(
    ctx: ActivityContext,
    input: CreateInstanceRecordInput,
) -> Result<CreateInstanceRecordOutput, String> {
    ctx.trace_info(format!(
        "Creating CMS record for user '{}' (k8s: {})",
        input.user_name, input.k8s_name
    ));

    let pool = get_pool().await?;
    let mut tx = pool.begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO toygres_cms.instances
        (user_name, k8s_name, namespace, postgres_version, storage_size_gb,
         use_load_balancer, dns_name, state, create_orchestration_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'creating', $8)
        ON CONFLICT (k8s_name) DO UPDATE
        SET user_name = EXCLUDED.user_name,
            namespace = EXCLUDED.namespace,
            postgres_version = EXCLUDED.postgres_version,
            storage_size_gb = EXCLUDED.storage_size_gb,
            use_load_balancer = EXCLUDED.use_load_balancer,
            dns_name = EXCLUDED.dns_name,
            updated_at = NOW()
        WHERE toygres_cms.instances.create_orchestration_id = EXCLUDED.create_orchestration_id
        RETURNING id
        "#
    )
    .bind(&input.user_name)
    .bind(&input.k8s_name)
    .bind(&input.namespace)
    .bind(&input.postgres_version)
    .bind(input.storage_size_gb)
    .bind(input.use_load_balancer)
    .bind(&input.dns_name)
    .bind(&input.orchestration_id)
    .fetch_optional(&mut *tx)
    .await;

    match insert_result {
        Ok(Some(row)) => {
            tx.commit().await.map_err(|e| format!("Failed to commit CMS record: {}", e))?;
            let id: Uuid = row.try_get("id")
                .map_err(|e| format!("Failed to read CMS record id: {}", e))?;
            ctx.trace_info(format!("CMS record stored: {}", id));
            Ok(CreateInstanceRecordOutput { instance_id: id })
        }
        Err(SqlxError::Database(db_err))
            if db_err.code().as_deref() == Some("23505")
                && db_err.constraint() == Some("idx_instances_dns_name_unique") =>
        {
            let dns_name = input.dns_name.clone().ok_or_else(|| {
                "DNS conflict detected but DNS name missing from input".to_string()
            })?;

            let conflict = sqlx::query(
                r#"
                SELECT id, k8s_name, user_name, create_orchestration_id
                FROM toygres_cms.instances
                WHERE dns_name = $1
                  AND dns_name NOT LIKE '__deleted_%'
                  AND state IN ('creating', 'running')
                FOR UPDATE
                "#
            )
            .bind(&dns_name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to inspect DNS conflict: {}", e))?;

            if let Some(row) = conflict {
                let owner_id: String = row.try_get("create_orchestration_id")
                    .map_err(|e| format!("Failed to read orchestration id: {}", e))?;
                let k8s_name: String = row.try_get("k8s_name")
                    .map_err(|e| format!("Failed to read k8s_name: {}", e))?;
                let user_name: String = row.try_get("user_name")
                    .map_err(|e| format!("Failed to read user_name: {}", e))?;
                let instance_id: Uuid = row.try_get("id")
                    .map_err(|e| format!("Failed to read instance id: {}", e))?;

                if owner_id == input.orchestration_id {
                    // Replay from same orchestration – treat as success
                    tx.commit().await.map_err(|e| format!("Failed to commit CMS record: {}", e))?;
                    ctx.trace_info(format!(
                        "Reusing CMS record {} (k8s: {}) for orchestration replay",
                        instance_id, k8s_name
                    ));
                    Ok(CreateInstanceRecordOutput { instance_id })
                } else {
                    tx.rollback().await.map_err(|e| format!("Failed to rollback after DNS conflict: {}", e))?;
                    Err(format!(
                        "DNS name '{}' is already reserved by instance '{}' (user: {})",
                        dns_name, k8s_name, user_name
                    ))
                }
            } else {
                tx.rollback().await.map_err(|e| format!("Failed to rollback after DNS conflict inspection: {}", e))?;
                Err("DNS name conflict detected but record was not found. Please retry.".to_string())
            }
        }
        Err(e) => {
            tx.rollback().await.map_err(|err| format!("Failed to rollback after error: {}", err))?;
            Err(format!("Failed to create CMS record: {}", e))
        }
        Ok(None) => Err("CMS insert did not return a record".to_string()),
    }
}

