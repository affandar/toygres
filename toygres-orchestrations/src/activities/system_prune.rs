/// System Prune Activity
///
/// Performs system-wide maintenance using duroxide's bulk management APIs:
/// - Deletes terminal orchestration instances older than configured hours
/// - Prunes all executions to keep only the current one
///
/// Uses ctx.get_client() to access duroxide management capabilities directly.

use duroxide::ActivityContext;
use duroxide::providers::management::{InstanceFilter, PruneOptions};

use crate::activity_types::{SystemPruneInput, SystemPruneOutput, PruneLogEntry};

pub const NAME: &str = "toygres-orchestrations::activity::system-prune";

pub async fn activity(
    ctx: ActivityContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "System prune activity starting (iteration {}, delete older than {} hours, keep {} executions)",
        input.iteration, input.delete_terminal_older_than_hours, input.keep_executions
    ));

    let client = ctx.get_client();
    let mut output = SystemPruneOutput {
        iteration: input.iteration,
        ..Default::default()
    };

    // Calculate cutoff time (N hours ago in milliseconds)
    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get current time: {}", e))?
        .as_millis() as u64
        - (input.delete_terminal_older_than_hours * 3600 * 1000);

    // Step 1: Delete terminal instances completed before cutoff
    // The bulk delete only affects terminal (Completed/Failed) instances
    ctx.trace_info(format!(
        "Deleting terminal instances completed before {} ({} hours ago)",
        cutoff_ms, input.delete_terminal_older_than_hours
    ));

    let delete_filter = InstanceFilter {
        completed_before: Some(cutoff_ms),
        limit: Some(1000), // Process up to 1000 instances per run
        ..Default::default()
    };

    let delete_result = match client.delete_instance_bulk(delete_filter).await {
        Ok(result) => {
            output.instances_deleted = result.instances_deleted;
            ctx.trace_info(format!(
                "Bulk delete complete: {} instances, {} executions, {} events deleted",
                result.instances_deleted, result.executions_deleted, result.events_deleted
            ));
            Some((result.instances_deleted, result.executions_deleted, result.events_deleted))
        }
        Err(e) => {
            ctx.trace_warn(format!("Bulk delete failed: {}", e));
            None
        }
    };

    // Step 2: Prune executions across all instances to keep only the latest
    ctx.trace_info(format!(
        "Pruning executions to keep only last {} execution(s)",
        input.keep_executions
    ));

    let prune_filter = InstanceFilter {
        limit: Some(1000), // Process up to 1000 instances per run
        ..Default::default()
    };

    let prune_options = PruneOptions {
        keep_last: Some(input.keep_executions as u32),
        ..Default::default()
    };

    let prune_result = match client.prune_executions_bulk(prune_filter, prune_options).await {
        Ok(result) => {
            output.instances_pruned = result.instances_processed;
            output.executions_pruned = result.executions_deleted;
            ctx.trace_info(format!(
                "Bulk prune complete: {} executions deleted across {} instances",
                result.executions_deleted, result.instances_processed
            ));
            Some((result.instances_processed, result.executions_deleted, result.events_deleted))
        }
        Err(e) => {
            ctx.trace_warn(format!("Bulk prune failed: {}", e));
            None
        }
    };

    // Note: bulk prune now includes running instances (duroxide 0.1.9+), only protecting current execution

    // Always log a summary entry for this run
    let timestamp = chrono::Utc::now().to_rfc3339();
    
    let (deleted_instances, deleted_execs, deleted_events) = delete_result.unwrap_or((0, 0, 0));
    let (pruned_instances, pruned_execs, pruned_events) = prune_result.unwrap_or((0, 0, 0));
    
    output.prune_log.push(PruneLogEntry {
        timestamp: timestamp.clone(),
        operation: "run_summary".to_string(),
        instance_id: format!("iteration-{}", input.iteration),
        orchestration_name: "system-pruner".to_string(),
        status: "completed".to_string(),
        details: format!(
            "Deleted {} instances ({} execs, {} events). Pruned {} execs across {} instances ({} events).",
            deleted_instances, deleted_execs, deleted_events,
            pruned_execs, pruned_instances, pruned_events
        ),
    });

    ctx.trace_info(format!(
        "System prune complete: {} instances deleted, {} executions pruned",
        output.instances_deleted, output.executions_pruned
    ));

    Ok(output)
}
