/// System Pruner Orchestration
///
/// A continuously-running system orchestration that performs maintenance:
/// - Deletes terminal (Completed/Failed) orchestration instances older than 6 hours
/// - Prunes all executions to keep only the current one (for long-running actors)
///
/// This orchestration uses the continue-as-new pattern and runs every 1 hour.
/// It logs all prune/delete operations for debugging.

use duroxide::OrchestrationContext;

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::system-pruner";
use std::time::Duration;

use crate::activity_types::{
    SystemPruneInput, SystemPruneOutput,
};

pub async fn system_pruner_orchestration(
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

/// System Pruner Orchestration v1.0.1
///
/// Changes from v1.0.0:
/// - Timer increased from 1 minute to 2 minutes
/// - Keeps 2 iterations of executions instead of 1
pub async fn system_pruner_1_0_1(
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

/// System Pruner Orchestration v1.0.2
///
/// Changes from v1.0.1:
/// - Timer increased from 2 minutes to 5 minutes
pub async fn system_pruner_1_0_2(
    ctx: OrchestrationContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "[v1.0.2] System pruner starting iteration {} (run_id: {})",
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
        "[v1.0.2] Prune iteration {} complete: {} instances deleted, {} executions pruned across {} instances",
        input.iteration,
        result.instances_deleted,
        result.executions_pruned,
        result.instances_pruned
    ));

    // Step 2: Wait 5 minutes before next iteration (v1.0.2: increased from 2 minutes)
    ctx.trace_info("[v1.0.2] Waiting 5 minutes before next prune cycle");
    ctx.schedule_timer(Duration::from_secs(300)).await;

    // Step 3: Continue as new for next iteration
    let next_input = SystemPruneInput {
        run_id: input.run_id.clone(),
        iteration: input.iteration + 1,
        delete_terminal_older_than_hours: input.delete_terminal_older_than_hours,
        keep_executions: 2, // Keep 2 iterations (same as v1.0.1)
    };

    let next_input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize next input: {}", e))?;

    ctx.continue_as_new(next_input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;

    Ok(result)
}

/// System Pruner Orchestration v1.0.3
///
/// Changes from v1.0.2:
/// - Uses system-prune-2 activity (simplified, relies on duroxide 0.1.9+ bulk APIs)
/// - Keeps 3 executions instead of 2 (for better history retention)
/// - No special self-pruning workaround needed
pub async fn system_pruner_1_0_3(
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

/// System Pruner Orchestration v1.0.4
///
/// Changes from v1.0.3:
/// - Timer reduced from 5 minutes to 1 minute (for faster iteration during testing)
/// - Still uses system-prune-2 activity
/// - Still keeps 3 executions
pub async fn system_pruner_1_0_4(
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