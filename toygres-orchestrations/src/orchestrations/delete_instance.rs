//! Delete PostgreSQL instance orchestration

use duroxide::OrchestrationContext;
use crate::types::{DeleteInstanceInput, DeleteInstanceOutput};
use crate::activity_names::activities;
use crate::activity_types::{
    DeletePostgresInput, DeletePostgresOutput,
    UpdateInstanceStateInput, UpdateInstanceStateOutput,
    FreeDnsNameInput, FreeDnsNameOutput,
    GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput,
    DeleteInstanceRecordInput, DeleteInstanceRecordOutput,
    RaiseEventInput, RaiseEventOutput,
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
    
    let cms_record = ctx
        .schedule_activity_typed::<GetInstanceByK8sNameInput, GetInstanceByK8sNameOutput>(
            activities::cms::GET_INSTANCE_BY_K8S_NAME,
            &GetInstanceByK8sNameInput {
                k8s_name: input.name.clone(),
            },
        )
        .into_activity_typed::<GetInstanceByK8sNameOutput>()
        .await
        .map_err(|e| format!("Failed to query CMS record: {}", e))?;
    
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
    
    let delete_output = ctx
        .schedule_activity_typed::<DeletePostgresInput, DeletePostgresOutput>(activities::DELETE_POSTGRES, &delete_input)
        .into_activity_typed::<DeletePostgresOutput>()
        .await?;
    
    ctx.trace_info(format!("Instance deletion complete (deleted: {})", delete_output.deleted));
    
    // Step 2: Signal the instance actor to stop (if it exists)
    if let Some(actor_id) = instance_actor_id {
        ctx.trace_info(format!("Sending InstanceDeleted signal to actor '{}'", actor_id));
        
        let raise_result = ctx
            .schedule_activity_typed::<RaiseEventInput, RaiseEventOutput>(
                activities::RAISE_EVENT,
                &RaiseEventInput {
                    instance_id: actor_id.clone(),
                    event_name: "InstanceDeleted".to_string(),
                    event_data: "{}".to_string(),
                },
            )
            .into_activity_typed::<RaiseEventOutput>()
            .await;
        
        match raise_result {
            Ok(_) => ctx.trace_info("Instance actor notified of deletion"),
            Err(e) => ctx.trace_warn(format!("Failed to notify instance actor: {}", e)),
        }
    }
    
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
            activities::cms::UPDATE_INSTANCE_STATE,
            &update_input,
        )
        .into_activity_typed::<UpdateInstanceStateOutput>()
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
            activities::cms::FREE_DNS_NAME,
            &FreeDnsNameInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .into_activity_typed::<FreeDnsNameOutput>()
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
            activities::cms::DELETE_INSTANCE_RECORD,
            &DeleteInstanceRecordInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .into_activity_typed::<DeleteInstanceRecordOutput>()
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

