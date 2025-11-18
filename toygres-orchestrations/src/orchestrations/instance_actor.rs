/// Instance Actor Orchestration
/// 
/// A continuously-running orchestration that performs per-instance operations:
/// - Health monitoring (every 30 seconds)
/// - Future: Auto-scaling, backups, maintenance tasks
/// 
/// This orchestration uses the continue-as-new pattern to prevent unbounded history growth.
/// Each iteration:
/// 1. Performs health check
/// 2. Records results in CMS
/// 3. Waits 30 seconds
/// 4. Continues-as-new (restarts with fresh history)
/// 
/// The orchestration exits gracefully when it detects the instance is deleted/deleting.

use duroxide::OrchestrationContext;

use crate::activity_names::activities;
use crate::activity_types::{
    GetInstanceConnectionInput, GetInstanceConnectionOutput,
    TestConnectionInput, TestConnectionOutput,
    RecordHealthCheckInput, RecordHealthCheckOutput,
    UpdateInstanceHealthInput, UpdateInstanceHealthOutput,
};
use crate::types::InstanceActorInput;

pub async fn instance_actor_orchestration(
    ctx: OrchestrationContext,
    input: InstanceActorInput,
) -> Result<(), String> {
    ctx.trace_info(format!(
        "Instance actor iteration for: {} (orchestration: {})",
        input.k8s_name, input.orchestration_id
    ));
    
    // Step 1: Get instance connection string from CMS
    let conn_info = ctx
        .schedule_activity_typed::<GetInstanceConnectionInput, GetInstanceConnectionOutput>(
            activities::cms::GET_INSTANCE_CONNECTION,
            &GetInstanceConnectionInput {
                k8s_name: input.k8s_name.clone(),
            },
        )
        .into_activity_typed::<GetInstanceConnectionOutput>()
        .await
        .map_err(|e| format!("Failed to get instance connection: {}", e))?;
    
    // Step 2: Check if instance still exists
    if !conn_info.found {
        ctx.trace_info("Instance no longer exists in CMS, stopping instance actor");
        // Complete successfully - instance is truly gone
        return Ok(());
    }
    
    // If instance is in "deleting" state, continue monitoring until it actually disappears
    // The delete orchestration will eventually remove the CMS record, triggering the above exit
    if let Some(state) = &conn_info.state {
        if state == "deleting" {
            ctx.trace_info("Instance is being deleted, will keep monitoring until removed from CMS");
            // Continue to monitor during deletion
        } else if state == "deleted" {
            // Shouldn't normally reach here, but if we do, wait for CMS record removal
            ctx.trace_info("Instance marked as deleted, waiting for CMS record removal");
        }
    }
    
    let connection_string = match conn_info.connection_string {
        Some(conn) => conn,
        None => {
            ctx.trace_warn("No connection string available yet, skipping health check");
            
            // Still continue-as-new to try again later
            ctx.schedule_timer(30000).into_timer().await; // 30 seconds = 30000 milliseconds
            ctx.trace_info("Restarting instance actor with continue-as-new");
            
            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize input: {}", e))?;
            ctx.continue_as_new(input_json);
            
            // Return immediately after continue_as_new
            return Ok(());
        }
    };
    
    // Step 3: Test connection and measure response time
    let start_time_ms = ctx.utcnow_ms().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;
    
    let health_result = ctx
        .schedule_activity_typed::<TestConnectionInput, TestConnectionOutput>(
            activities::TEST_CONNECTION,
            &TestConnectionInput {
                connection_string: connection_string.clone(),
            },
        )
        .into_activity_typed::<TestConnectionOutput>()
        .await;
    
    let end_time_ms = ctx.utcnow_ms().await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let response_time_ms = (end_time_ms - start_time_ms) as i32;
    
    // Step 4: Determine health status and extract details
    let (status, postgres_version, error_message) = match health_result {
        Ok(output) => {
            ctx.trace_info(format!("Health check passed ({}ms)", response_time_ms));
            ("healthy", Some(output.version), None)
        }
        Err(e) => {
            ctx.trace_warn(format!("Health check failed: {}", e));
            ("unhealthy", None, Some(e.to_string()))
        }
    };
    
    // Step 5: Record health check in database
    let _record = ctx
        .schedule_activity_typed::<RecordHealthCheckInput, RecordHealthCheckOutput>(
            activities::cms::RECORD_HEALTH_CHECK,
            &RecordHealthCheckInput {
                k8s_name: input.k8s_name.clone(),
                status: status.to_string(),
                postgres_version,
                response_time_ms: Some(response_time_ms),
                error_message,
            },
        )
        .into_activity_typed::<RecordHealthCheckOutput>()
        .await
        .map_err(|e| format!("Failed to record health check: {}", e))?;
    
    // Step 6: Update instance health status
    let _update = ctx
        .schedule_activity_typed::<UpdateInstanceHealthInput, UpdateInstanceHealthOutput>(
            activities::cms::UPDATE_INSTANCE_HEALTH,
            &UpdateInstanceHealthInput {
                k8s_name: input.k8s_name.clone(),
                health_status: status.to_string(),
            },
        )
        .into_activity_typed::<UpdateInstanceHealthOutput>()
        .await
        .map_err(|e| format!("Failed to update instance health: {}", e))?;
    
    ctx.trace_info(format!("Health check complete, status: {}", status));
    
    // Step 7: Wait 30 seconds before next check
    ctx.schedule_timer(30000).into_timer().await; // 30 seconds = 30000 milliseconds
    
    ctx.trace_info("Restarting instance actor with continue-as-new");
    
    // Step 8: Continue as new to prevent unbounded history growth
    // This ends the current execution and starts a fresh one with the same input
    let input_json = serde_json::to_string(&input)
        .map_err(|e| format!("Failed to serialize input: {}", e))?;
    
    ctx.continue_as_new(input_json);
    
    // Return immediately after continue_as_new (the runtime will restart this orchestration)
    Ok(())
}

