use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};

use std::time::Duration;
use crate::types::{DeleteInstanceInput, DeleteInstanceOutput};
use crate::activities::{self, cms};
use crate::activity_types::{
    DeletePostgresInput, DeletePostgresOutput,
    UpdateInstanceStateInput,
    GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput,
    SendExternalEventInput, SendExternalEventOutput,
};

/// v1.0.3: Same as v1.0.2 but with custom status reporting for live progress.
pub async fn delete_instance_1_0_3_orchestration(
    ctx: OrchestrationContext,
    input: DeleteInstanceInput,
) -> Result<DeleteInstanceOutput, String> {
    ctx.set_custom_status("Initializing deletion");
    ctx.trace_info(format!(
        "[v1.0.3] Deleting PostgreSQL instance: {} (orchestration: {})",
        input.name, input.orchestration_id
    ));

    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());

    // Get CMS record with retry for resilience
    ctx.set_custom_status("Looking up instance record");
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
        ctx.set_custom_status("Marking instance as deleting");
        let update_input = UpdateInstanceStateInput {
            k8s_name: input.name.clone(),
            state: "deleting".to_string(),
            ip_connection_string: None,
            dns_connection_string: None,
            external_ip: None,
            delete_orchestration_id: Some(input.orchestration_id.clone()),
            message: Some("Deletion requested".to_string()),
        };
        super::update_cms_state_strict(&ctx, update_input).await?;
    } else {
        ctx.trace_info("[v1.0.3] CMS record not found, proceeding with best-effort cleanup");
    }

    // Send InstanceDeleted signal to actor BEFORE deleting resources
    if let Some(ref actor_id) = instance_actor_id {
        ctx.set_custom_status("Signaling instance actor to stop");
        ctx.trace_info(format!(
            "[v1.0.3] Sending InstanceDeleted signal to actor '{}'",
            actor_id
        ));

        let signal_input = SendExternalEventInput {
            instance_id: actor_id.clone(),
            event_name: "InstanceDeleted".to_string(),
            payload: "{}".to_string(),
        };

        let signal_result = ctx
            .schedule_activity_typed::<SendExternalEventInput, SendExternalEventOutput>(
                activities::send_external_event::NAME,
                &signal_input,
            )
            .await;

        match signal_result {
            Ok(output) => {
                if output.sent {
                    ctx.trace_info("[v1.0.3] InstanceDeleted signal sent successfully");
                } else {
                    ctx.trace_info("[v1.0.3] InstanceDeleted signal could not be sent (actor may already be stopped)");
                }
            }
            Err(e) => {
                ctx.trace_warn(format!("[v1.0.3] Failed to send InstanceDeleted signal: {}", e));
            }
        }
    } else {
        ctx.trace_info("[v1.0.3] No instance actor recorded, skipping signal");
    }

    // Step 1: Delete PostgreSQL resources
    ctx.set_custom_status("Deleting Kubernetes resources");
    ctx.trace_info("[v1.0.3] Step 1: Deleting PostgreSQL from Kubernetes");
    let delete_input = DeletePostgresInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
    };

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

    ctx.trace_info(format!("[v1.0.3] Instance deletion complete (deleted: {})", delete_output.deleted));

    // Mark as deleted state
    ctx.set_custom_status("Updating CMS record");
    let update_input = UpdateInstanceStateInput {
        k8s_name: input.name.clone(),
        state: "deleted".to_string(),
        ip_connection_string: None,
        dns_connection_string: None,
        external_ip: None,
        delete_orchestration_id: Some(input.orchestration_id.clone()),
        message: Some(format!("Deleted (resources deleted: {})", delete_output.deleted)),
    };
    super::update_cms_state_strict(&ctx, update_input).await?;

    // Step 3: Delete the CMS record
    ctx.set_custom_status("Cleaning up CMS records");
    ctx.trace_info("[v1.0.3] Removing CMS record");
    super::delete_cms_record(&ctx, &input.name).await;

    super::free_dns_name(&ctx, &input.name).await;

    ctx.set_custom_status("Deletion complete");

    Ok(DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    })
}
