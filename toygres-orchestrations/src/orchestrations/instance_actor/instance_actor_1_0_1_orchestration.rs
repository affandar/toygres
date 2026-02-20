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

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};

use std::time::Duration;

use crate::activities::{self, cms};
use crate::activity_types::{
    GetInstanceConnectionInput, GetInstanceConnectionOutput,
    TestConnectionInput, TestConnectionOutput,
    RecordHealthCheckInput, RecordHealthCheckOutput,
    UpdateInstanceHealthInput, UpdateInstanceHealthOutput,
};
use crate::types::InstanceActorInput;

/// v1.0.1: Never-die actor — all failures handled gracefully with continue-as-new
pub async fn instance_actor_1_0_1_orchestration(
    ctx: OrchestrationContext,
    input: InstanceActorInput,
) -> Result<(), String> {
    ctx.trace_info(format!(
        "[v1.0.1] Instance actor iteration for: {} (orchestration: {})",
        input.k8s_name, input.orchestration_id
    ));
    
    // Step 1: Get instance connection string from CMS
    // On failure, wait and continue-as-new instead of dying
    let conn_info = match ctx
        .schedule_activity_with_retry_typed::<GetInstanceConnectionInput, GetInstanceConnectionOutput>(
                cms::get_instance_connection::NAME,
                &GetInstanceConnectionInput {
                    k8s_name: input.k8s_name.clone(),
                },
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Exponential {
                    base: Duration::from_secs(2),
                    multiplier: 2.0,
                    max: Duration::from_secs(10),
                })
                .with_timeout(Duration::from_secs(30)),
        )
        .await
    {
        Ok(info) => info,
        Err(e) => {
            ctx.trace_warn(format!("[v1.0.1] Failed to get instance connection: {}. Will retry next cycle.", e));
            ctx.schedule_timer(Duration::from_secs(30)).await;
            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize input: {}", e))?;
            ctx.continue_as_new(input_json).await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(());
        }
    };
    
    // Step 2: Check if instance still exists
    if !conn_info.found {
        ctx.trace_info("[v1.0.1] Instance no longer exists in CMS, stopping instance actor");
        return Ok(());
    }
    
    if let Some(state) = &conn_info.state {
        if state == "deleting" {
            ctx.trace_info("[v1.0.1] Instance is being deleted, will keep monitoring until removed from CMS");
        } else if state == "deleted" {
            ctx.trace_info("[v1.0.1] Instance marked as deleted, waiting for CMS record removal");
        }
    }
    
    let connection_string = match conn_info.connection_string {
        Some(conn) => conn,
        None => {
            ctx.trace_warn("[v1.0.1] No connection string available yet, skipping health check");
            
            ctx.schedule_timer(Duration::from_secs(30)).await;
            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize input: {}", e))?;
            ctx.continue_as_new(input_json).await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(());
        }
    };
    
    // Step 3: Test connection and measure response time
    let start_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;
    
    let health_result = ctx
        .schedule_activity_with_retry_typed::<TestConnectionInput, TestConnectionOutput>(
            activities::test_connection::NAME,
            &TestConnectionInput {
                connection_string: connection_string.clone(),
                k8s_name: None,
            },
            RetryPolicy::new(3)
                .with_backoff(BackoffStrategy::Linear {
                    base: Duration::from_secs(1),
                    max: Duration::from_secs(5),
                })
                .with_timeout(Duration::from_secs(30)),
        )
        .await;
    
    let end_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let response_time_ms = end_time.duration_since(start_time)
        .map_err(|e| format!("Failed to calculate duration: {}", e))?
        .as_millis() as i32;
    
    // Step 4: Determine health status and extract details
    let (status, postgres_version, error_message) = match health_result {
        Ok(output) => {
            ctx.trace_info(format!("[v1.0.1] Health check passed ({}ms)", response_time_ms));
            ("healthy", Some(output.version), None)
        }
        Err(e) => {
            ctx.trace_warn(format!("[v1.0.1] Health check failed: {}", e));
            ("unhealthy", None, Some(e.to_string()))
        }
    };
    
    // Step 5: Record health check — log warning on failure but don't die
    if let Err(e) = ctx
        .schedule_activity_typed::<RecordHealthCheckInput, RecordHealthCheckOutput>(
            cms::record_health_check::NAME,
            &RecordHealthCheckInput {
                k8s_name: input.k8s_name.clone(),
                status: status.to_string(),
                postgres_version,
                response_time_ms: Some(response_time_ms),
                error_message,
            },
        )
        .await
    {
        ctx.trace_warn(format!("[v1.0.1] Failed to record health check: {}. Continuing.", e));
    }
    
    // Step 6: Update instance health status — log warning on failure but don't die
    if let Err(e) = ctx
        .schedule_activity_typed::<UpdateInstanceHealthInput, UpdateInstanceHealthOutput>(
            cms::update_instance_health::NAME,
            &UpdateInstanceHealthInput {
                k8s_name: input.k8s_name.clone(),
                health_status: status.to_string(),
            },
        )
        .await
    {
        ctx.trace_warn(format!("[v1.0.1] Failed to update instance health: {}. Continuing.", e));
    }
    
    ctx.trace_info(format!("[v1.0.1] Health check complete, status: {}", status));
    
    // Step 7: Wait for either 30 seconds OR deletion signal (whichever comes first)
    let timer = ctx.schedule_timer(Duration::from_secs(30));
    let deletion_signal = ctx.schedule_wait("InstanceDeleted");
    
    use duroxide::Either2;
    match ctx.select2(timer, deletion_signal).await {
        Either2::First(()) => {
            // Timer fired
        }
        Either2::Second(_) => {
            ctx.trace_info("[v1.0.1] Received InstanceDeleted signal, stopping instance actor gracefully");
            return Ok(());
        }
    }
    
    // Step 8: Continue as new
    ctx.trace_info("[v1.0.1] Health check cycle complete, restarting instance actor with continue-as-new");
    let input_json = serde_json::to_string(&input)
        .map_err(|e| format!("Failed to serialize input: {}", e))?;
    ctx.continue_as_new(input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;
    Ok(())
}
