use duroxide::ActivityContext;

use crate::activity_types::{RecordInstanceActorInput, RecordInstanceActorOutput};

use super::get_pool;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::cms-record-instance-actor";

pub async fn activity(
    ctx: ActivityContext,
    input: RecordInstanceActorInput,
) -> Result<RecordInstanceActorOutput, String> {
    let pool = get_pool().await?;
    
    let result = sqlx::query(
        r#"
        UPDATE toygres_cms.instances
        SET instance_actor_orchestration_id = $2, updated_at = NOW()
        WHERE k8s_name = $1
        "#
    )
    .bind(&input.k8s_name)
    .bind(&input.instance_actor_orchestration_id)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to record instance actor orchestration ID: {}", e))?;
    
    let recorded = result.rows_affected() > 0;
    
    if recorded {
        ctx.trace_info(format!(
            "Instance actor orchestration ID recorded: {}",
            input.instance_actor_orchestration_id
        ));
    } else {
        ctx.trace_warn(format!("Instance not found in CMS: {}", input.k8s_name));
    }
    
    Ok(RecordInstanceActorOutput { recorded })
}

