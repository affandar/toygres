use duroxide::ActivityContext;

use crate::activity_types::{DeleteInstanceRecordInput, DeleteInstanceRecordOutput};

use super::get_pool;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::cms-delete-instance-record";

pub async fn activity(
    ctx: ActivityContext,
    input: DeleteInstanceRecordInput,
) -> Result<DeleteInstanceRecordOutput, String> {
    let pool = get_pool().await?;
    
    let result = sqlx::query(
        r#"
        DELETE FROM toygres_cms.instances
        WHERE k8s_name = $1
        "#
    )
    .bind(&input.k8s_name)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to delete CMS record: {}", e))?;
    
    let deleted = result.rows_affected() > 0;
    
    if deleted {
        ctx.trace_info(format!("CMS record deleted: {}", input.k8s_name));
    } else {
        ctx.trace_warn(format!("CMS record not found for deletion: {}", input.k8s_name));
    }
    
    Ok(DeleteInstanceRecordOutput { deleted })
}

