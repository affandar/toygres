/// Get Batch Status Activity
///
/// Queries duroxide for the status of multiple orchestration instances.
/// Used by the batch-create orchestration to monitor child create-instance orchestrations.

use duroxide::ActivityContext;

use crate::activity_types::{GetBatchStatusInput, GetBatchStatusOutput, BatchStatusEntry};

pub const NAME: &str = "toygres-orchestrations::activity::get-batch-status";

pub async fn activity(
    ctx: ActivityContext,
    input: GetBatchStatusInput,
) -> Result<GetBatchStatusOutput, String> {
    ctx.trace_info(format!(
        "Checking status of {} orchestrations",
        input.orchestration_ids.len()
    ));

    let client = ctx.get_client();
    let mut entries = Vec::with_capacity(input.orchestration_ids.len());

    for orch_id in &input.orchestration_ids {
        let entry = match client.get_orchestration_status(orch_id).await {
            Ok(duroxide::OrchestrationStatus::Running { custom_status, .. }) => {
                BatchStatusEntry {
                    orchestration_id: orch_id.clone(),
                    status: "Running".to_string(),
                    custom_status,
                    error: None,
                }
            }
            Ok(duroxide::OrchestrationStatus::Completed { custom_status, .. }) => {
                BatchStatusEntry {
                    orchestration_id: orch_id.clone(),
                    status: "Completed".to_string(),
                    custom_status,
                    error: None,
                }
            }
            Ok(duroxide::OrchestrationStatus::Failed { details, custom_status, .. }) => {
                let error_msg = details.display_message();
                // Truncate long error messages for UX
                let error_trimmed = if error_msg.len() > 200 {
                    format!("{}...", &error_msg[..200])
                } else {
                    error_msg
                };
                BatchStatusEntry {
                    orchestration_id: orch_id.clone(),
                    status: "Failed".to_string(),
                    custom_status,
                    error: Some(error_trimmed),
                }
            }
            Ok(duroxide::OrchestrationStatus::NotFound) => {
                BatchStatusEntry {
                    orchestration_id: orch_id.clone(),
                    status: "NotFound".to_string(),
                    custom_status: None,
                    error: None,
                }
            }
            Err(e) => {
                BatchStatusEntry {
                    orchestration_id: orch_id.clone(),
                    status: "Unknown".to_string(),
                    custom_status: None,
                    error: Some(format!("Failed to query status: {}", e)),
                }
            }
        };
        entries.push(entry);
    }

    Ok(GetBatchStatusOutput { entries })
}
