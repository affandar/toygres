use duroxide::ActivityContext;
use sqlx::Row;
use tokio::time::{sleep, Duration};

use crate::activity_types::{GetInstanceConnectionInput, GetInstanceConnectionOutput};

use super::get_pool;

pub async fn get_instance_connection_activity(
    _ctx: ActivityContext,
    input: GetInstanceConnectionInput,
) -> Result<GetInstanceConnectionOutput, String> {
    let pool = get_pool().await?;
    
    // Retry logic: 3 attempts with 3s delay between retries
    let max_attempts = 3;
    let retry_delay = Duration::from_secs(3);
    
    let mut last_error = String::new();
    
    for attempt in 1..=max_attempts {
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
        .await;
        
        match record {
            Ok(Some(row)) => {
                let connection_string: Option<String> = row.try_get("connection_string").ok();
                let state: Option<String> = row.try_get("state").ok();
                
                return Ok(GetInstanceConnectionOutput {
                    found: true,
                    connection_string,
                    state,
                });
            }
            Ok(None) => {
                return Ok(GetInstanceConnectionOutput {
                    found: false,
                    connection_string: None,
                    state: None,
                });
            }
            Err(e) => {
                last_error = format!("Failed to query instance connection: {}", e);
                
                if attempt < max_attempts {
                    // Wait before retrying
                    sleep(retry_delay).await;
                }
            }
        }
    }
    
    // All retries exhausted
    Err(format!("{} (after {} attempts)", last_error, max_attempts))
}

