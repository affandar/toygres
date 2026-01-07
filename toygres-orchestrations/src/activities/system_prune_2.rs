/// System Prune Activity v2
///
/// Simplified version that relies entirely on duroxide 0.1.9+ bulk APIs:
/// - Bulk delete: Deletes terminal orchestration instances older than configured hours
/// - Bulk prune: Prunes executions across ALL instances (including running ones)
///
/// Key improvement over v1: No special self-pruning workaround needed.
/// The bulk prune API now includes running instances, only protecting the current execution.

use duroxide::ActivityContext;
use duroxide::providers::management::{InstanceFilter, PruneOptions};

use crate::activity_types::{PruneLogEntry, SystemPruneInput, SystemPruneOutput};

pub const NAME: &str = "toygres-orchestrations::activity::system-prune-2";

pub async fn activity(
    ctx: ActivityContext,
    input: SystemPruneInput,
) -> Result<SystemPruneOutput, String> {
    ctx.trace_info(format!(
        "[v2] System prune starting (iteration {}, delete_older_than={} hours, keep_executions={})",
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
    ctx.trace_info(format!(
        "[v2] Deleting terminal instances completed before {} ({} hours ago)",
        cutoff_ms, input.delete_terminal_older_than_hours
    ));

    let delete_filter = InstanceFilter {
        completed_before: Some(cutoff_ms),
        limit: Some(1000),
        ..Default::default()
    };

    let delete_result = match client.delete_instance_bulk(delete_filter).await {
        Ok(result) => {
            output.instances_deleted = result.instances_deleted;
            ctx.trace_info(format!(
                "[v2] Bulk delete: {} instances, {} executions, {} events",
                result.instances_deleted, result.executions_deleted, result.events_deleted
            ));
            Some((
                result.instances_deleted,
                result.executions_deleted,
                result.events_deleted,
            ))
        }
        Err(e) => {
            ctx.trace_warn(format!("[v2] Bulk delete failed: {}", e));
            None
        }
    };

    // Step 2: Prune executions across ALL instances (including running ones like system-pruner)
    // duroxide 0.1.9+ includes running instances in bulk prune, only protecting current execution
    ctx.trace_info(format!(
        "[v2] Pruning executions to keep last {} across all instances",
        input.keep_executions
    ));

    let prune_filter = InstanceFilter {
        limit: Some(1000),
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
                "[v2] Bulk prune: {} executions across {} instances ({} events)",
                result.executions_deleted, result.instances_processed, result.events_deleted
            ));
            Some((
                result.instances_processed,
                result.executions_deleted,
                result.events_deleted,
            ))
        }
        Err(e) => {
            ctx.trace_warn(format!("[v2] Bulk prune failed: {}", e));
            None
        }
    };

    // Build summary log
    let timestamp = chrono::Utc::now().to_rfc3339();
    let (deleted_instances, deleted_execs, deleted_events) = delete_result.unwrap_or((0, 0, 0));
    let (pruned_instances, pruned_execs, pruned_events) = prune_result.unwrap_or((0, 0, 0));

    output.prune_log.push(PruneLogEntry {
        timestamp,
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
        "[v2] Complete: {} deleted, {} pruned across {} instances",
        output.instances_deleted, output.executions_pruned, output.instances_pruned
    ));

    Ok(output)
}
