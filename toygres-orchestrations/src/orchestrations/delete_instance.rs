//! Delete PostgreSQL instance orchestration

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::delete-instance";
use std::time::Duration;
use crate::types::{DeleteInstanceInput, DeleteInstanceOutput};
use crate::activities::{self, cms};
use crate::activity_types::{
    DeletePostgresInput, DeletePostgresOutput,
    UpdateInstanceStateInput, UpdateInstanceStateOutput,
    FreeDnsNameInput, FreeDnsNameOutput,
    GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput,
    DeleteInstanceRecordInput, DeleteInstanceRecordOutput,
};

pub async fn delete_instance_orchestration(
    ctx: OrchestrationContext,
    input: DeleteInstanceInput,
) -> Result<DeleteInstanceOutput, String> {
    ctx.trace_info(format!(
        "Deleting PostgreSQL instance: {} (orchestration: {})",
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
        update_cms_state(&ctx, update_input).await;
    } else {
        ctx.trace_info("CMS record not found, proceeding with best-effort cleanup");
    }
    
    // Step 0.5: Note that instance actor will be signaled after deletion
    if let Some(ref actor_id) = instance_actor_id {
        ctx.trace_info(format!(
            "Instance actor '{}' will receive deletion signal after cleanup",
            actor_id
        ));
    }
    
    // Step 1: Delete PostgreSQL resources
    ctx.trace_info("Step 1: Deleting PostgreSQL from Kubernetes");
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
    
    ctx.trace_info(format!("Instance deletion complete (deleted: {})", delete_output.deleted));
    
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
    update_cms_state(&ctx, update_input).await;
    
    // Step 3: Delete the CMS record
    ctx.trace_info("Removing CMS record");
    delete_cms_record(&ctx, &input.name).await;
    
    free_dns_name(&ctx, &input.name).await;
    
    // Return output
    Ok(DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    })
}

async fn update_cms_state(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) {
    if let Err(err) = ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .await
    {
        ctx.trace_warn(format!("Failed to update CMS state: {}", err));
    }
}

async fn free_dns_name(
    ctx: &OrchestrationContext,
    k8s_name: &str,
) {
    if let Err(err) = ctx
        .schedule_activity_typed::<FreeDnsNameInput, FreeDnsNameOutput>(
            cms::free_dns_name::NAME,
            &FreeDnsNameInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .await
    {
        ctx.trace_warn(format!("Failed to free DNS name: {}", err));
    }
}

async fn delete_cms_record(
    ctx: &OrchestrationContext,
    k8s_name: &str,
) {
    ctx.trace_info("Deleting CMS record (triggers instance actor completion)");
    
    if let Err(err) = ctx
        .schedule_activity_typed::<DeleteInstanceRecordInput, DeleteInstanceRecordOutput>(
            cms::delete_instance_record::NAME,
            &DeleteInstanceRecordInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .await
    {
        ctx.trace_warn(format!("Failed to delete CMS record: {}", err));
    } else {
        ctx.trace_info("CMS record deleted, instance actor will complete on next iteration");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_delete_instance_input_serialization() {
        let input = DeleteInstanceInput {
            name: "test-pg".to_string(),
            namespace: Some("toygres".to_string()),
            orchestration_id: "delete-test".to_string(),
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: DeleteInstanceInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, parsed);
    }
    
    #[test]
    fn test_delete_instance_output_serialization() {
        let output = DeleteInstanceOutput {
            instance_name: "test-pg".to_string(),
            deleted: true,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: DeleteInstanceOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, parsed);
    }
}

// ============================================================================
// v1.0.1 - Propagate CMS update errors
// ============================================================================

/// v1.0.1: Deletes PostgreSQL instance with proper CMS error propagation
/// 
/// Changes from v1.0.0:
/// - CMS state update errors are now propagated (fail orchestration if CMS fails)
pub async fn delete_instance_1_0_1(
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
        update_cms_state_v1_0_1(&ctx, update_input).await?;
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
    update_cms_state_v1_0_1(&ctx, update_input).await?;
    
    // Step 3: Delete the CMS record
    ctx.trace_info("[v1.0.1] Removing CMS record");
    delete_cms_record(&ctx, &input.name).await;
    
    free_dns_name(&ctx, &input.name).await;
    
    // Return output
    Ok(DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    })
}

/// v1.0.1: CMS state update that returns Result
async fn update_cms_state_v1_0_1(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) -> Result<(), String> {
    ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .await
        .map_err(|e| format!("Failed to update CMS state: {}", e))?;
    Ok(())
}

// ============================================================================
// v1.0.2 - Send InstanceDeleted signal to actor before deleting CMS record
// ============================================================================

use crate::activity_types::{SendExternalEventInput, SendExternalEventOutput};

/// v1.0.2: Deletes PostgreSQL instance and signals actor to stop gracefully
///
/// Changes from v1.0.1:
/// - Sends InstanceDeleted external event to instance actor before deleting CMS record
/// - Actor receives signal and exits gracefully instead of timing out
pub async fn delete_instance_1_0_2(
    ctx: OrchestrationContext,
    input: DeleteInstanceInput,
) -> Result<DeleteInstanceOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.2] Deleting PostgreSQL instance: {} (orchestration: {})",
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
        update_cms_state_v1_0_1(&ctx, update_input).await?;
    } else {
        ctx.trace_info("[v1.0.2] CMS record not found, proceeding with best-effort cleanup");
    }

    // v1.0.2: Send InstanceDeleted signal to actor BEFORE deleting resources
    if let Some(ref actor_id) = instance_actor_id {
        ctx.trace_info(format!(
            "[v1.0.2] Sending InstanceDeleted signal to actor '{}'",
            actor_id
        ));

        let signal_input = SendExternalEventInput {
            instance_id: actor_id.clone(),
            event_name: "InstanceDeleted".to_string(),
            payload: "{}".to_string(),
        };

        // Best effort - don't fail if signal can't be sent (actor might already be done)
        let signal_result = ctx
            .schedule_activity_typed::<SendExternalEventInput, SendExternalEventOutput>(
                activities::send_external_event::NAME,
                &signal_input,
            )
            .await;

        match signal_result {
            Ok(output) => {
                if output.sent {
                    ctx.trace_info("[v1.0.2] InstanceDeleted signal sent successfully");
                } else {
                    ctx.trace_info("[v1.0.2] InstanceDeleted signal could not be sent (actor may already be stopped)");
                }
            }
            Err(e) => {
                ctx.trace_warn(format!("[v1.0.2] Failed to send InstanceDeleted signal: {}", e));
            }
        }
    } else {
        ctx.trace_info("[v1.0.2] No instance actor recorded, skipping signal");
    }

    // Step 1: Delete PostgreSQL resources
    ctx.trace_info("[v1.0.2] Step 1: Deleting PostgreSQL from Kubernetes");
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

    ctx.trace_info(format!("[v1.0.2] Instance deletion complete (deleted: {})", delete_output.deleted));

    // Mark as deleted state
    let update_input = UpdateInstanceStateInput {
        k8s_name: input.name.clone(),
        state: "deleted".to_string(),
        ip_connection_string: None,
        dns_connection_string: None,
        external_ip: None,
        delete_orchestration_id: Some(input.orchestration_id.clone()),
        message: Some(format!("Deleted (resources deleted: {})", delete_output.deleted)),
    };
    update_cms_state_v1_0_1(&ctx, update_input).await?;

    // Step 3: Delete the CMS record
    ctx.trace_info("[v1.0.2] Removing CMS record");
    delete_cms_record(&ctx, &input.name).await;

    free_dns_name(&ctx, &input.name).await;

    // Return output
    Ok(DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    })
}

