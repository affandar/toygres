use duroxide::ActivityContext;

use crate::activity_types::{UpdateInstanceHealthInput, UpdateInstanceHealthOutput};

use super::get_pool;

pub async fn update_instance_health_activity(
    _ctx: ActivityContext,
    input: UpdateInstanceHealthInput,
) -> Result<UpdateInstanceHealthOutput, String> {
    let pool = get_pool().await?;
    
    let result = sqlx::query(
        r#"
        UPDATE toygres_cms.instances
        SET health_status = $2::health_status, updated_at = NOW()
        WHERE k8s_name = $1
          AND state = 'running'
        "#
    )
    .bind(&input.k8s_name)
    .bind(&input.health_status)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to update instance health: {}", e))?;
    
    Ok(UpdateInstanceHealthOutput {
        updated: result.rows_affected() > 0,
    })
}

