use duroxide::OrchestrationContext;

use std::time::Duration;

use crate::activity_types::{
    SystemPruneInput, SystemPruneOutput,
};

/// System Pruner Orchestration v1.0.3
///
/// Changes from v1.0.2:
/// - Uses system-prune-2 activity (simplified, relies on duroxide 0.1.9+ bulk APIs)
/// - Keeps 3 executions instead of 2 (for better history retention)
/// - No special self-pruning workaround needed
pub async fn system_pruner_1_0_3_orchestration(
    ctx: OrchestrationContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.3] System pruner starting iteration {} (run_id: {})",
        input.iteration, input.run_id
    ));

    // Use the new system-prune-2 activity (no self-prune workaround needed)
    let prune_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 3, // v1.0.3: keep 3 executions for all eternal orchestrations
    };

    let result = ctx
        .schedule_activity_typed::<SystemPruneInput, SystemPruneOutput>(
            crate::activities::system_prune_2::NAME,
            &prune_input,
        )
        .await
        .map_err(|e| format!("System prune activity failed: {}", e))?;

    ctx.trace_info(format!(
        "[v1.0.3] Prune iteration {} complete: {} instances deleted, {} executions pruned across {} instances",
        input.iteration,
        result.instances_deleted,
        result.executions_pruned,
        result.instances_pruned
    ));

    // Wait 5 minutes before next iteration (same as v1.0.2)
    ctx.trace_info("[v1.0.3] Waiting 5 minutes before next prune cycle");
    ctx.schedule_timer(Duration::from_secs(300)).await;

    // Continue as new for next iteration
    let next_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration + 1,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 3, // v1.0.3: keep 3 executions
    };

    let next_input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize next input: {}", e))?;

    ctx.continue_as_new(next_input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;

    Ok(result)
}
