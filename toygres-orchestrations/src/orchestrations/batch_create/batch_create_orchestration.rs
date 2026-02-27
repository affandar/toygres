/// Batch Create Orchestration v1.0.0
///
/// Phase 1: Start N create-instance orchestrations (detached, fire-and-forget)
/// Phase 2: Poll every 15s via get-batch-status activity, update custom_status
/// Phase 3: When all terminal, return summary with errors
/// Uses continue-as-new every 100 polls to bound history.

use duroxide::OrchestrationContext;
use std::time::Duration;

use crate::orchestrations::create_instance;
use crate::activities;
use crate::types::{
    BatchCreateInput, BatchCreateOutput, BatchInstance, BatchError,
    CreateInstanceInput,
};
use crate::activity_types::{GetBatchStatusInput, GetBatchStatusOutput};

const POLL_INTERVAL_SECS: u64 = 15;
const CONTINUE_AS_NEW_THRESHOLD: u32 = 100;

pub async fn batch_create_1_0_0_orchestration(
    ctx: OrchestrationContext,
    mut input: BatchCreateInput,
) -> Result<BatchCreateOutput, String> {
    // Phase 1: Start child orchestrations (only if instances list is empty — first run)
    if input.instances.is_empty() {
        ctx.set_custom_status(&make_status_json(input.count, 0, 0, 0, &[]));
        ctx.trace_info(format!(
            "[batch v1.0.0] Starting {} create-instance orchestrations (base: {})",
            input.count, input.base_name
        ));

        for i in 1..=input.count {
            let user_name = format!("{}{}", input.base_name, i);
            let suffix = format!("{:08x}", {
                // Deterministic: use ctx.utc_now() to derive a unique suffix
                let now = ctx.utc_now().await
                    .map_err(|e| format!("Failed to get time: {}", e))?;
                let nanos = now.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                // Mix instance index into the timestamp for uniqueness
                (nanos.wrapping_mul(6364136223846793005).wrapping_add(i as u64)) & 0xFFFFFFFF
            });
            let k8s_name = format!("{}-{}", user_name, suffix);
            let orchestration_id = format!("create-{}", k8s_name);

            let create_input = CreateInstanceInput {
                user_name: user_name.clone(),
                name: k8s_name.clone(),
                password: input.password.clone(),
                postgres_version: Some(input.postgres_version.clone()),
                storage_size_gb: Some(input.storage_size_gb),
                use_load_balancer: Some(input.use_load_balancer),
                dns_label: Some(user_name.clone()),
                namespace: Some(input.namespace.clone()),
                orchestration_id: orchestration_id.clone(),
                image_type: input.image_type.clone(),
                source_image_id: None,
                runtime_image_id: input.runtime_image_id.clone(),
                image_override: input.image_override.clone(),
            };

            let input_json = serde_json::to_string(&create_input)
                .map_err(|e| format!("Failed to serialize create input: {}", e))?;

            ctx.schedule_orchestration(
                create_instance::NAME,
                &orchestration_id,
                input_json,
            );

            input.instances.push(BatchInstance {
                user_name,
                k8s_name,
                orchestration_id,
            });
        }

        ctx.trace_info(format!(
            "[batch v1.0.0] All {} orchestrations started, entering monitoring phase",
            input.count
        ));
    } else {
        ctx.trace_info(format!(
            "[batch v1.0.0] Resuming monitoring: {} instances, poll #{}, {} errors so far",
            input.instances.len(),
            input.polls_completed,
            input.errors.len(),
        ));
    }

    // Phase 2: Poll until all terminal
    loop {
        ctx.schedule_timer(Duration::from_secs(POLL_INTERVAL_SECS)).await;

        let orch_ids: Vec<String> = input.instances.iter()
            .map(|inst| inst.orchestration_id.clone())
            .collect();

        let status_output = ctx
            .schedule_activity_typed::<GetBatchStatusInput, GetBatchStatusOutput>(
                activities::get_batch_status::NAME,
                &GetBatchStatusInput { orchestration_ids: orch_ids },
            )
            .await
            .map_err(|e| format!("Failed to get batch status: {}", e))?;

        let mut completed = 0u32;
        let mut failed = 0u32;
        let mut creating = 0u32;

        // Build a lookup from orchestration_id → user_name
        let name_lookup: std::collections::HashMap<&str, &str> = input.instances.iter()
            .map(|inst| (inst.orchestration_id.as_str(), inst.user_name.as_str()))
            .collect();

        // Collect new errors (only ones we haven't seen yet)
        let known_errors: std::collections::HashSet<String> = input.errors.iter()
            .map(|e| e.instance_name.clone())
            .collect();

        for entry in &status_output.entries {
            match entry.status.as_str() {
                "Completed" => completed += 1,
                "Failed" => {
                    failed += 1;
                    let instance_name = name_lookup
                        .get(entry.orchestration_id.as_str())
                        .unwrap_or(&"unknown");
                    if !known_errors.contains(*instance_name) {
                        input.errors.push(BatchError {
                            instance_name: instance_name.to_string(),
                            error: entry.error.clone()
                                .or(entry.custom_status.clone())
                                .unwrap_or_else(|| "Unknown error".to_string()),
                        });
                    }
                }
                _ => creating += 1,
            }
        }

        let status_json = make_status_json(input.count, completed, failed, creating, &input.errors);
        ctx.set_custom_status(&status_json);

        ctx.trace_info(format!(
            "[batch v1.0.0] Poll #{}: {}/{} completed, {} failed, {} creating",
            input.polls_completed + 1, completed, input.count, failed, creating
        ));

        input.polls_completed += 1;

        // Check if all terminal
        if completed + failed >= input.count {
            ctx.trace_info(format!(
                "[batch v1.0.0] All instances terminal. {} completed, {} failed",
                completed, failed
            ));
            return Ok(BatchCreateOutput {
                total: input.count,
                completed,
                failed,
                errors: input.errors,
            });
        }

        // Continue-as-new to bound history
        if input.polls_completed >= CONTINUE_AS_NEW_THRESHOLD {
            ctx.trace_info("[batch v1.0.0] Continue-as-new after 100 polls");
            input.polls_completed = 0;
            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize batch state: {}", e))?;
            ctx.continue_as_new(input_json).await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(BatchCreateOutput {
                total: input.count,
                completed,
                failed,
                errors: input.errors,
            });
        }
    }
}

/// Build the custom status JSON string for display
fn make_status_json(
    total: u32,
    completed: u32,
    failed: u32,
    creating: u32,
    errors: &[BatchError],
) -> String {
    // Only include the last 10 errors to keep status manageable
    let recent_errors: Vec<&BatchError> = errors.iter().rev().take(10).collect();
    let errors_json: Vec<serde_json::Value> = recent_errors.iter().map(|e| {
        serde_json::json!({
            "instance": e.instance_name,
            "error": e.error,
        })
    }).collect();

    serde_json::json!({
        "total": total,
        "completed": completed,
        "failed": failed,
        "creating": creating,
        "errors": errors_json,
    }).to_string()
}
