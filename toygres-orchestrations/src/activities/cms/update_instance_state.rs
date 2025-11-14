use duroxide::ActivityContext;
use sqlx::{Row, types::JsonValue};
use uuid::Uuid;

use crate::activity_types::{UpdateInstanceStateInput, UpdateInstanceStateOutput};

use super::get_pool;

pub async fn update_instance_state_activity(
    ctx: ActivityContext,
    input: UpdateInstanceStateInput,
) -> Result<UpdateInstanceStateOutput, String> {
    let pool = get_pool().await?;
    let mut tx = pool.begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let record = sqlx::query(
        r#"
        SELECT id, state::text as state
        FROM toygres_cms.instances
        WHERE k8s_name = $1
        FOR UPDATE
        "#
    )
    .bind(&input.k8s_name)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Failed to load CMS record: {}", e))?;

    let Some(row) = record else {
        tx.rollback().await.map_err(|e| format!("Failed to rollback after missing instance: {}", e))?;
        ctx.trace_warn(format!("CMS record not found for {}", input.k8s_name));
        return Ok(UpdateInstanceStateOutput { updated: false, previous_state: None });
    };

    let instance_id: Uuid = row.try_get("id")
        .map_err(|e| format!("Failed to read instance id: {}", e))?;
    let previous_state: String = row.try_get("state")
        .map_err(|e| format!("Failed to read previous state: {}", e))?;

    sqlx::query(
        r#"
        UPDATE toygres_cms.instances
        SET state = $2::instance_state,
            ip_connection_string = COALESCE($3, ip_connection_string),
            dns_connection_string = COALESCE($4, dns_connection_string),
            external_ip = COALESCE($5, external_ip),
            delete_orchestration_id = COALESCE($6, delete_orchestration_id),
            updated_at = NOW(),
            deleted_at = CASE WHEN $2 = 'deleted' THEN NOW() ELSE deleted_at END
        WHERE id = $1
        "#
    )
    .bind(instance_id)
    .bind(&input.state)
    .bind(&input.ip_connection_string)
    .bind(&input.dns_connection_string)
    .bind(&input.external_ip)
    .bind(&input.delete_orchestration_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update CMS record: {}", e))?;

    if previous_state != input.state {
        ctx.trace_info(format!(
            "Instance '{}' state transition: {} → {}",
            input.k8s_name, previous_state, input.state
        ));

        sqlx::query(
            r#"
            INSERT INTO toygres_cms.instance_events
            (instance_id, event_type, old_state, new_state, message, metadata)
            VALUES ($1, 'state_change', $2, $3, $4, $5)
            "#
        )
        .bind(instance_id)
        .bind(&previous_state)
        .bind(&input.state)
        .bind(&input.message)
        .bind::<Option<JsonValue>>(None)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to insert instance event: {}", e))?;
    }

    tx.commit().await.map_err(|e| format!("Failed to commit CMS update: {}", e))?;

    Ok(UpdateInstanceStateOutput {
        updated: true,
        previous_state: Some(previous_state),
    })
}

