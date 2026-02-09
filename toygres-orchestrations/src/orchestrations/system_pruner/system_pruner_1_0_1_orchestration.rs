use duroxide::OrchestrationContext;

use std::time::Duration;

use crate::activity_types::{
    SystemPruneInput, SystemPruneOutput,
};

/// System Pruner Orchestration v1.0.1
///
/// Changes from v1.0.0:
/// - Timer increased from 1 minute to 2 minutes
/// - Keeps 2 iterations of executions instead of 1
pub async fn system_pruner_1_0_1_orchestration(
    ctx: OrchestrationContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.1] System pruner starting iteration {} (run_id: {})",
        input.iteration, input.run_id
    ));

    // Step 1: Prune and delete old instances
    let result = ctx
        .schedule_activity_typed::<SystemPruneInput, SystemPruneOutput>(
            crate::activities::system_prune::NAME,
            &input,
        )
        .await
        .map_err(|e| format!("System prune activity failed: {}", e))?;

    ctx.trace_info(format!(
        "[v1.0.1] Prune iteration {} complete: {} instances deleted, {} executions pruned across {} instances",
        input.iteration,
        result.instances_deleted,
        result.executions_pruned,
        result.instances_pruned
    ));

    // Step 2: Wait 2 minutes before next iteration (v1.0.1: increased from 1 minute)
    ctx.trace_info("[v1.0.1] Waiting 2 minutes before next prune cycle");
    ctx.schedule_timer(Duration::from_secs(120)).await;

    // Step 3: Continue as new for next iteration
    // Note: keep_executions is 2 in v1.0.1 (vs 1 in v1.0.0)
    let next_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration + 1,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 2, // v1.0.1: keep 2 iterations instead of 1
    };

    let next_input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize next input: {}", e))?;

    ctx.continue_as_new(next_input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;

    Ok(result)
}
