use duroxide::ActivityContext;
use sqlx::Row;

use crate::activity_types::{FreeDnsNameInput, FreeDnsNameOutput};

use super::get_pool;

pub async fn free_dns_name_activity(
    ctx: ActivityContext,
    input: FreeDnsNameInput,
) -> Result<FreeDnsNameOutput, String> {
    let pool = get_pool().await?;

    let current = sqlx::query(
        r#"
        SELECT dns_name
        FROM toygres_cms.instances
        WHERE k8s_name = $1
        "#
    )
    .bind(&input.k8s_name)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("Failed to fetch DNS name: {}", e))?;

    let Some(row) = current else {
        ctx.trace_info(format!("No CMS record found for {}, nothing to free", input.k8s_name));
        return Ok(FreeDnsNameOutput { freed: false });
    };

    let dns_name: Option<String> = row.try_get("dns_name")
        .map_err(|e| format!("Failed to read DNS name: {}", e))?;

    let Some(existing) = dns_name else {
        ctx.trace_info("DNS name already empty, no action needed");
        return Ok(FreeDnsNameOutput { freed: false });
    };

    if existing.starts_with("__deleted_") {
        ctx.trace_info("DNS name already marked as deleted");
        return Ok(FreeDnsNameOutput { freed: false });
    }

    sqlx::query(
        r#"
        UPDATE toygres_cms.instances
        SET dns_name = CONCAT('__deleted_', dns_name),
            updated_at = NOW()
        WHERE k8s_name = $1
        "#
    )
    .bind(&input.k8s_name)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to free DNS name: {}", e))?;

    ctx.trace_info(format!("Freed DNS name for {}", input.k8s_name));
    Ok(FreeDnsNameOutput { freed: true })
}

