use duroxide::ActivityContext;
use sqlx::Row;

use crate::activity_types::{GetInstanceConnectionInput, GetInstanceConnectionOutput};

use super::get_pool;

pub async fn get_instance_connection_activity(
    _ctx: ActivityContext,
    input: GetInstanceConnectionInput,
) -> Result<GetInstanceConnectionOutput, String> {
    let pool = get_pool().await?;
    
    let record = sqlx::query(
        r#"
        SELECT 
            COALESCE(dns_connection_string, ip_connection_string) as connection_string,
            state::text
        FROM toygres_cms.instances
        WHERE k8s_name = $1
        LIMIT 1
        "#
    )
    .bind(&input.k8s_name)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("Failed to query instance connection: {}", e))?;
    
    match record {
        Some(row) => {
            let connection_string: Option<String> = row.try_get("connection_string").ok();
            let state: Option<String> = row.try_get("state").ok();
            
            Ok(GetInstanceConnectionOutput {
                found: true,
                connection_string,
                state,
            })
        }
        None => Ok(GetInstanceConnectionOutput {
            found: false,
            connection_string: None,
            state: None,
        }),
    }
}

