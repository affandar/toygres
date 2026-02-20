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

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy, Either2};

use std::time::Duration;

use crate::activities::{self, cms};
use crate::activity_types::{
    GetInstanceConnectionInput, GetInstanceConnectionOutput,
    TestConnectionInput, TestConnectionOutput,
    RecordHealthCheckInput, RecordHealthCheckOutput,
    UpdateInstanceHealthInput, UpdateInstanceHealthOutput,
};
use crate::types::InstanceActorInput;

/// v1.0.2: Session-affine health checks with persistent connection pooling
///
/// Changes from v1.0.1:
/// - test-connection activity uses session affinity (session_id = k8s_name)
///   so the same worker handles each instance across continue-as-new cycles,
///   enabling worker-local connection pooling
/// - Retry logic moved into the activity itself (3 attempts with 5s connect timeout)
/// - Removed orchestration-level retry policy on test-connection
pub async fn instance_actor_1_0_2_orchestration(
    ctx: OrchestrationContext,
    input: InstanceActorInput,
) -> Result<(), String> {
    ctx.trace_info(format!(
        "[v1.0.2] Instance actor iteration for: {} (orchestration: {})",
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
            ctx.trace_warn(format!("[v1.0.2] Failed to get instance connection: {}. Will retry next cycle.", e));
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
        ctx.trace_info("[v1.0.2] Instance no longer exists in CMS, stopping instance actor");
        return Ok(());
    }
    
    if let Some(state) = &conn_info.state {
        if state == "deleting" {
            ctx.trace_info("[v1.0.2] Instance is being deleted, will keep monitoring until removed from CMS");
        } else if state == "deleted" {
            ctx.trace_info("[v1.0.2] Instance marked as deleted, waiting for CMS record removal");
        }
    }
    
    let connection_string = match conn_info.connection_string {
        Some(conn) => conn,
        None => {
            ctx.trace_warn("[v1.0.2] No connection string available yet, skipping health check");
            
            ctx.schedule_timer(Duration::from_secs(30)).await;
            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize input: {}", e))?;
            ctx.continue_as_new(input_json).await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(());
        }
    };
    
    // Step 3: Test connection with session affinity + durable retry
    // Session ID = k8s_name ensures the same worker handles this instance
    // across continue-as-new cycles, enabling worker-local connection pooling.
    // Manual retry loop mirrors duroxide's schedule_activity_with_retry pattern
    // since there's no schedule_activity_on_session_with_retry API yet.
    let start_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;
    
    let max_attempts = 3u32;
    let mut health_result: Result<TestConnectionOutput, String> = Err("no attempts".to_string());
    
    for attempt in 1..=max_attempts {
        let attempt_result = {
            // Race activity vs 30s per-attempt timeout
            let test_input = TestConnectionInput {
                connection_string: connection_string.clone(),
                k8s_name: Some(input.k8s_name.clone()),
            };
            let activity = ctx.schedule_activity_on_session_typed::<TestConnectionInput, TestConnectionOutput>(
                activities::test_connection::NAME,
                &test_input,
                &input.k8s_name,
            );
            let deadline = ctx.schedule_timer(Duration::from_secs(30));
            
            match ctx.select2(activity, deadline).await {
                Either2::First(result) => result,
                Either2::Second(()) => Err("timeout: activity timed out".to_string()),
            }
        };
        
        match attempt_result {
            Ok(output) => {
                health_result = Ok(output);
                break;
            }
            Err(ref e) => {
                if attempt < max_attempts {
                    ctx.trace_warn(format!(
                        "[v1.0.2] test-connection attempt {}/{} failed: {}. Retrying...",
                        attempt, max_attempts, e
                    ));
                    // Linear backoff: 2s, 4s
                    let delay = Duration::from_secs(2 * attempt as u64);
                    ctx.schedule_timer(delay).await;
                }
                health_result = attempt_result;
            }
        }
    }
    
    let end_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get end time: {}", e))?;
    let response_time_ms = end_time.duration_since(start_time)
        .map_err(|e| format!("Failed to calculate duration: {}", e))?
        .as_millis() as i32;
    
    // Step 4: Determine health status and extract details
    let (status, postgres_version, error_message) = match health_result {
        Ok(output) => {
            ctx.trace_info(format!("[v1.0.2] Health check passed ({}ms)", response_time_ms));
            ("healthy", Some(output.version), None)
        }
        Err(e) => {
            ctx.trace_warn(format!("[v1.0.2] Health check failed: {}", e));
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
        ctx.trace_warn(format!("[v1.0.2] Failed to record health check: {}. Continuing.", e));
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
        ctx.trace_warn(format!("[v1.0.2] Failed to update instance health: {}. Continuing.", e));
    }
    
    ctx.trace_info(format!("[v1.0.2] Health check complete, status: {}", status));
    
    // Step 7: Wait for either 30 seconds OR deletion signal (whichever comes first)
    let timer = ctx.schedule_timer(Duration::from_secs(30));
    let deletion_signal = ctx.schedule_wait("InstanceDeleted");
    
    match ctx.select2(timer, deletion_signal).await {
        Either2::First(()) => {
            // Timer fired
        }
        Either2::Second(_) => {
            ctx.trace_info("[v1.0.2] Received InstanceDeleted signal, stopping instance actor gracefully");
            return Ok(());
        }
    }
    
    // Step 8: Continue as new
    ctx.trace_info("[v1.0.2] Health check cycle complete, restarting instance actor with continue-as-new");
    let input_json = serde_json::to_string(&input)
        .map_err(|e| format!("Failed to serialize input: {}", e))?;
    ctx.continue_as_new(input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;
    Ok(())
}
