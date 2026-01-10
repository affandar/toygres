use duroxide::ActivityContext;
use sqlx::Row;
use uuid::Uuid;

use crate::activity_types::{
    CmsInstanceRecord,
    GetInstanceByK8sNameInput,
    GetInstanceByK8sNameOutput,
};

use super::get_pool;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::cms-get-instance-by-k8s-name";

pub async fn activity(
    _ctx: ActivityContext,
    input: GetInstanceByK8sNameInput,
) -> Result<GetInstanceByK8sNameOutput, String> {
    let pool = get_pool().await?;

    let record = sqlx::query(
        r#"
        SELECT id, user_name, k8s_name, namespace, state::text as state, dns_name, 
               instance_actor_orchestration_id, postgres_version, storage_size_gb,
               image_type::text as image_type
        FROM toygres_cms.instances
        WHERE k8s_name = $1
        "#
    )
    .bind(&input.k8s_name)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("Failed to fetch CMS record: {}", e))?;

    if let Some(row) = record {
        let rec = CmsInstanceRecord {
            id: row.try_get::<Uuid, _>("id").map_err(|e| format!("Failed to read id: {}", e))?,
            user_name: row.try_get("user_name").map_err(|e| format!("Failed to read user_name: {}", e))?,
            k8s_name: row.try_get("k8s_name").map_err(|e| format!("Failed to read k8s_name: {}", e))?,
            namespace: row.try_get("namespace").map_err(|e| format!("Failed to read namespace: {}", e))?,
            state: row.try_get("state").map_err(|e| format!("Failed to read state: {}", e))?,
            dns_name: row.try_get("dns_name").ok(),
            postgres_version: row.try_get("postgres_version").unwrap_or_else(|_| "16".to_string()),
            storage_size_gb: row.try_get("storage_size_gb").unwrap_or(10),
            image_type: row.try_get("image_type").unwrap_or_else(|_| "stock".to_string()),
        };
        let instance_actor_orchestration_id: Option<String> = row.try_get("instance_actor_orchestration_id").ok();
        
        Ok(GetInstanceByK8sNameOutput {
            found: true,
            record: Some(rec),
            instance_actor_orchestration_id,
        })
    } else {
        Ok(GetInstanceByK8sNameOutput {
            found: false,
            record: None,
            instance_actor_orchestration_id: None,
        })
    }
}

