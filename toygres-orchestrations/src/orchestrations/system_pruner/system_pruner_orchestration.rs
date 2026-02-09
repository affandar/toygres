use duroxide::OrchestrationContext;

use std::time::Duration;

use crate::activity_types::{
    SystemPruneInput, SystemPruneOutput,
};

/// System Pruner Orchestration v1.0.4
///
/// Changes from v1.0.3:
/// - Timer reduced from 5 minutes to 1 minute (for faster iteration during testing)
/// - Still uses system-prune-2 activity
/// - Still keeps 3 executions
pub async fn system_pruner_1_0_4_orchestration(
    ctx: OrchestrationContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.4] System pruner starting iteration {} (run_id: {})",
        input.iteration, input.run_id
    ));

    // Use the system-prune-2 activity (same as v1.0.3)
    let prune_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 3, // Keep 3 executions for all eternal orchestrations
    };

    let result = ctx
        .schedule_activity_typed::<SystemPruneInput, SystemPruneOutput>(
            crate::activities::system_prune_2::NAME,
            &prune_input,
        )
        .await
        .map_err(|e| format!("System prune activity failed: {}", e))?;

    ctx.trace_info(format!(
        "[v1.0.4] Prune iteration {} complete: {} instances deleted, {} executions pruned across {} instances",
        input.iteration,
        result.instances_deleted,
        result.executions_pruned,
        result.instances_pruned
    ));

    // Wait 1 minute before next iteration (v1.0.4: reduced from 5 minutes)
    ctx.trace_info("[v1.0.4] Waiting 1 minute before next prune cycle");
    ctx.schedule_timer(Duration::from_secs(60)).await;

    // Continue as new for next iteration
    let next_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration + 1,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 3, // Keep 3 executions
    };

    let next_input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize next input: {}", e))?;

    ctx.continue_as_new(next_input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;

    Ok(result)
}
