use duroxide::OrchestrationContext;

use std::time::Duration;

use crate::activity_types::{
    SystemPruneInput, SystemPruneOutput,
};

/// System Pruner Orchestration v1.0.5
///
/// Changes from v1.0.4:
/// - Custom status reporting for live visibility into prune operations
pub async fn system_pruner_1_0_5_orchestration(
    ctx: OrchestrationContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.set_custom_status(&format!("Pruning (iteration {})", input.iteration));
    ctx.trace_info(format!(
        "[v1.0.5] System pruner starting iteration {} (run_id: {})",
        input.iteration, input.run_id
    ));

    let prune_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 3,
    };

    let result = ctx
        .schedule_activity_typed::<SystemPruneInput, SystemPruneOutput>(
            crate::activities::system_prune_2::NAME,
            &prune_input,
        )
        .await
        .map_err(|e| format!("System prune activity failed: {}", e))?;

    ctx.set_custom_status(&format!(
        "Iteration {}: {} deleted, {} pruned",
        input.iteration, result.instances_deleted, result.executions_pruned
    ));
    ctx.trace_info(format!(
        "[v1.0.5] Prune iteration {} complete: {} instances deleted, {} executions pruned across {} instances",
        input.iteration,
        result.instances_deleted,
        result.executions_pruned,
        result.instances_pruned
    ));

    // Wait 1 minute before next iteration
    ctx.set_custom_status("Waiting for next cycle");
    ctx.trace_info("[v1.0.5] Waiting 1 minute before next prune cycle");
    ctx.schedule_timer(Duration::from_secs(60)).await;

    // Continue as new for next iteration
    let next_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration + 1,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 3,
    };

    let next_input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize next input: {}", e))?;

    ctx.continue_as_new(next_input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;

    Ok(result)
}
