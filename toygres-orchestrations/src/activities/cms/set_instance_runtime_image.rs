use duroxide::ActivityContext;
use crate::activity_types::{SetInstanceRuntimeImageInput, SetInstanceRuntimeImageOutput};
use super::get_pool;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::cms-set-instance-runtime-image";

pub async fn activity(
    ctx: ActivityContext,
    input: SetInstanceRuntimeImageInput,
) -> Result<SetInstanceRuntimeImageOutput, String> {
    ctx.trace_info(format!(
        "Setting runtime_image_id for instance {} -> {}",
        input.k8s_name, input.runtime_image_id
    ));

    let pool = get_pool().await?;

    let result = sqlx::query(
        r#"
        UPDATE toygres_cms.instances
        SET runtime_image_id = $1,
            updated_at = NOW()
        WHERE k8s_name = $2
        "#,
    )
    .bind(input.runtime_image_id)
    .bind(&input.k8s_name)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to set runtime_image_id: {}", e))?;

    Ok(SetInstanceRuntimeImageOutput {
        updated: result.rows_affected() > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_activity_name_constant() {
        assert!(NAME.contains("cms-set-instance-runtime-image"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let input = SetInstanceRuntimeImageInput {
            k8s_name: "test-abc".to_string(),
            runtime_image_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: SetInstanceRuntimeImageInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
}
