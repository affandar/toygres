use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};

use std::time::Duration;
use crate::types::{DeleteInstanceInput, DeleteInstanceOutput};
use crate::activities;
use crate::activities::cms;
use crate::activity_types::{
    DeletePostgresInput, DeletePostgresOutput,
    UpdateInstanceStateInput,
    GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput,
};

/// v1.0.1: Deletes PostgreSQL instance with proper CMS error propagation
/// 
/// Changes from v1.0.0:
/// - CMS state update errors are now propagated (fail orchestration if CMS fails)
pub async fn delete_instance_1_0_1_orchestration(
    ctx: OrchestrationContext,
    input: DeleteInstanceInput,
) -> Result<DeleteInstanceOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.1] Deleting PostgreSQL instance: {} (orchestration: {})",
        input.name, input.orchestration_id
    ));
    
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    
    // Get CMS record with retry for resilience
    let cms_record = ctx
        .schedule_activity_with_retry_typed::<GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput>(
            cms::get_instance_by_k8s_name::NAME,
            &GetInstanceByK8sNameInput {
                k8s_name: input.name.clone(),
            },
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Fixed {
                    delay: Duration::from_secs(2),
                })
                .with_timeout(Duration::from_secs(10)),
        )
        .await
        .map_err(|e| format!("Failed to query CMS record after retries: {}", e))?;
    
    // Store instance actor ID for later use
    let instance_actor_id = cms_record.instance_actor_orchestration_id.clone();
    
    if cms_record.found {
        let update_input = UpdateInstanceStateInput {
            k8s_name: input.name.clone(),
            state: "deleting".to_string(),
            ip_connection_string: None,
            dns_connection_string: None,
            external_ip: None,
            delete_orchestration_id: Some(input.orchestration_id.clone()),
            message: Some("Deletion requested".to_string()),
        };
        // v1.0.1: Propagate CMS update error
        super::update_cms_state_strict(&ctx, update_input).await?;
    } else {
        ctx.trace_info("[v1.0.1] CMS record not found, proceeding with best-effort cleanup");
    }
    
    // Step 0.5: Note that instance actor will be signaled after deletion
    if let Some(ref actor_id) = instance_actor_id {
        ctx.trace_info(format!(
            "[v1.0.1] Instance actor '{}' will receive deletion signal after cleanup",
            actor_id
        ));
    }
    
    // Step 1: Delete PostgreSQL resources
    ctx.trace_info("[v1.0.1] Step 1: Deleting PostgreSQL from Kubernetes");
    let delete_input = DeletePostgresInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
    };
    
    // Delete K8s resources with retry - API calls can be flaky
    let delete_output = ctx
        .schedule_activity_with_retry_typed::<DeletePostgresInput, DeletePostgresOutput>(
            activities::delete_postgres::NAME,
            &delete_input,
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(1),
                    multiplier: 2.0,
                    max: Duration::from_secs(10),
                })
                .with_timeout(Duration::from_secs(60)),
        )
        .await?;
    
    ctx.trace_info(format!("[v1.0.1] Instance deletion complete (deleted: {})", delete_output.deleted));
    
    // Mark as deleted state (instance actor will detect this and exit gracefully)
    let update_input = UpdateInstanceStateInput {
        k8s_name: input.name.clone(),
        state: "deleted".to_string(),
        ip_connection_string: None,
        dns_connection_string: None,
        external_ip: None,
        delete_orchestration_id: Some(input.orchestration_id.clone()),
        message: Some(format!("Deleted (resources deleted: {})", delete_output.deleted)),
    };
    // v1.0.1: Propagate CMS update error
    super::update_cms_state_strict(&ctx, update_input).await?;
    
    // Step 3: Delete the CMS record
    ctx.trace_info("[v1.0.1] Removing CMS record");
    super::delete_cms_record(&ctx, &input.name).await;
    
    super::free_dns_name(&ctx, &input.name).await;
    
    // Return output
    Ok(DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    })
}
