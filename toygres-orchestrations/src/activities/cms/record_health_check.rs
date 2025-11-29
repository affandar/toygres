use duroxide::ActivityContext;
use sqlx::Row;

use crate::activity_types::{RecordHealthCheckInput, RecordHealthCheckOutput};

use super::get_pool;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::cms-record-health-check";

pub async fn activity(
    ctx: ActivityContext,
    input: RecordHealthCheckInput,
) -> Result<RecordHealthCheckOutput, String> {
    let pool = get_pool().await?;
    
    let result = sqlx::query(
        r#"
        INSERT INTO toygres_cms.instance_health_checks 
        (instance_id, status, postgres_version, response_time_ms, error_message, checked_at)
        SELECT i.id, $2, $3, $4, $5, NOW()
        FROM toygres_cms.instances i
        WHERE i.k8s_name = $1
        RETURNING id
        "#
    )
    .bind(&input.k8s_name)
    .bind(&input.status)
    .bind(&input.postgres_version)
    .bind(input.response_time_ms)
    .bind(&input.error_message)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("Failed to insert health check: {}", e))?;
    
    match result {
        Some(row) => {
            let check_id: i64 = row.try_get("id")
                .map_err(|e| format!("Failed to read check_id: {}", e))?;
            
            ctx.trace_info(format!("Health check recorded: {} (check_id: {})", input.status, check_id));
            
            Ok(RecordHealthCheckOutput {
                recorded: true,
                check_id,
            })
        }
        None => {
            ctx.trace_warn(format!("Instance not found in CMS: {}", input.k8s_name));
            Ok(RecordHealthCheckOutput {
                recorded: false,
                check_id: 0,
            })
        }
    }
}

