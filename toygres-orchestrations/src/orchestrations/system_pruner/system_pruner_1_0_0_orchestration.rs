use duroxide::OrchestrationContext;

use std::time::Duration;

use crate::activity_types::{
    SystemPruneInput, SystemPruneOutput,
};

pub async fn system_pruner_1_0_0_orchestration(
    ctx: OrchestrationContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "System pruner starting iteration {} (run_id: {})",
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
        "Prune iteration {} complete: {} instances deleted, {} executions pruned across {} instances",
        input.iteration,
        result.instances_deleted,
        result.executions_pruned,
        result.instances_pruned
    ));

    // Step 2: Wait 1 minute before next iteration
    ctx.trace_info("Waiting 1 minute before next prune cycle");
    ctx.schedule_timer(Duration::from_secs(60)).await;

    // Step 3: Continue as new for next iteration
    let next_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration + 1,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: input.keep_executions,
    };

    let next_input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize next input: {}", e))?;

    ctx.continue_as_new(next_input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;

    Ok(result)
}
