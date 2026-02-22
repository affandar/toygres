/// Instance Actor Orchestration v1.0.3
///
/// Changes from v1.0.2:
/// - Reports health status via custom_status as JSON:
///   { "health": "healthy", "response_time_ms": 150, "postgres_version": "..." }
/// - Listens on "query" event queue for ad-hoc SQL query requests
/// - Query results are reported via custom_status
/// - Uses select3 to race: timer vs deletion signal vs query event

use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy, Either2, Either3};

use std::time::Duration;

use crate::activities::{self, cms};
use crate::activity_types::{
    GetInstanceConnectionInput, GetInstanceConnectionOutput,
    TestConnectionInput, TestConnectionOutput,
    RecordHealthCheckInput, RecordHealthCheckOutput,
    UpdateInstanceHealthInput, UpdateInstanceHealthOutput,
    ExecuteQueryInput, ExecuteQueryOutput,
};
use crate::types::InstanceActorInput;

/// Query request sent via event queue
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryRequest {
    pub query: String,
    pub request_id: String,
}

/// Custom status structure for the instance actor
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActorCustomStatus {
    pub health: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postgres_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_result: Option<QueryResult>,
}

/// Query result embedded in custom status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    pub request_id: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn instance_actor_1_0_3_orchestration(
    ctx: OrchestrationContext,
    input: InstanceActorInput,
) -> Result<(), String> {
    ctx.trace_info(format!(
        "[v1.0.3] Instance actor iteration for: {} (orchestration: {})",
        input.k8s_name, input.orchestration_id
    ));

    // Step 1: Get instance connection string from CMS
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
            ctx.set_custom_status(r#"{"health":"unknown","error":"Failed to get connection info"}"#);
            ctx.trace_warn(format!("[v1.0.3] Failed to get instance connection: {}. Will retry next cycle.", e));
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
        ctx.set_custom_status(r#"{"health":"stopped","reason":"Instance no longer exists"}"#);
        ctx.trace_info("[v1.0.3] Instance no longer exists in CMS, stopping instance actor");
        return Ok(());
    }

    if let Some(state) = &conn_info.state {
        if state == "deleting" {
            ctx.trace_info("[v1.0.3] Instance is being deleted, will keep monitoring until removed from CMS");
        } else if state == "deleted" {
            ctx.trace_info("[v1.0.3] Instance marked as deleted, waiting for CMS record removal");
        }
    }

    let connection_string = match conn_info.connection_string {
        Some(conn) => conn,
        None => {
            ctx.set_custom_status(r#"{"health":"unknown","reason":"No connection string available"}"#);
            ctx.trace_warn("[v1.0.3] No connection string available yet, skipping health check");

            ctx.schedule_timer(Duration::from_secs(30)).await;
            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize input: {}", e))?;
            ctx.continue_as_new(input_json).await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(());
        }
    };

    // Step 3: Test connection with session affinity + retry
    let start_time = ctx.utc_now().await
        .map_err(|e| format!("Failed to get start time: {}", e))?;

    let max_attempts = 3u32;
    let mut health_result: Result<TestConnectionOutput, String> = Err("no attempts".to_string());

    for attempt in 1..=max_attempts {
        let attempt_result = {
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
                        "[v1.0.3] test-connection attempt {}/{} failed: {}. Retrying...",
                        attempt, max_attempts, e
                    ));
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

    // Step 4: Determine health status and set custom status
    let (status, postgres_version, error_message) = match health_result {
        Ok(output) => {
            ctx.trace_info(format!("[v1.0.3] Health check passed ({}ms)", response_time_ms));
            ("healthy", Some(output.version), None)
        }
        Err(e) => {
            ctx.trace_warn(format!("[v1.0.3] Health check failed: {}", e));
            ("unhealthy", None, Some(e.to_string()))
        }
    };

    // Set custom status with health info (include last query result if present)
    let last_query: Option<QueryResult> = input.last_query_result.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let custom_status = ActorCustomStatus {
        health: status.to_string(),
        response_time_ms: Some(response_time_ms),
        postgres_version: postgres_version.clone(),
        query_result: last_query,
    };
    if let Ok(status_json) = serde_json::to_string(&custom_status) {
        ctx.set_custom_status(&status_json);
    }

    // Step 5: Record health check
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
        ctx.trace_warn(format!("[v1.0.3] Failed to record health check: {}. Continuing.", e));
    }

    // Step 6: Update instance health status
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
        ctx.trace_warn(format!("[v1.0.3] Failed to update instance health: {}. Continuing.", e));
    }

    ctx.trace_info(format!("[v1.0.3] Health check complete, status: {}", status));

    // Step 7: Wait for timer, deletion signal, OR query request (whichever comes first)
    let timer = ctx.schedule_timer(Duration::from_secs(30));
    let deletion_signal = ctx.schedule_wait("InstanceDeleted");
    let query_event = ctx.dequeue_event_typed::<QueryRequest>("query");

    match ctx.select3(timer, deletion_signal, query_event).await {
        Either3::First(()) => {
            // Timer fired — proceed to continue-as-new
        }
        Either3::Second(_) => {
            ctx.set_custom_status(r#"{"health":"stopped","reason":"Instance deleted"}"#);
            ctx.trace_info("[v1.0.3] Received InstanceDeleted signal, stopping instance actor gracefully");
            return Ok(());
        }
        Either3::Third(query_request) => {
            // Execute the ad-hoc query
            ctx.trace_info(format!("[v1.0.3] Received query request: {}", query_request.request_id));

            let query_output = ctx
                .schedule_activity_on_session_typed::<ExecuteQueryInput, ExecuteQueryOutput>(
                    activities::execute_query::NAME,
                    &ExecuteQueryInput {
                        connection_string: connection_string.clone(),
                        query: query_request.query,
                        k8s_name: Some(input.k8s_name.clone()),
                    },
                    &input.k8s_name,
                )
                .await;

            let query_result = match query_output {
                Ok(output) => QueryResult {
                    request_id: query_request.request_id,
                    columns: output.columns,
                    rows: output.rows,
                    row_count: output.row_count,
                    success: output.success,
                    error: output.error,
                },
                Err(e) => QueryResult {
                    request_id: query_request.request_id,
                    columns: Vec::new(),
                    rows: Vec::new(),
                    row_count: 0,
                    success: false,
                    error: Some(e),
                },
            };

            // Update custom status with query result
            let status_with_query = ActorCustomStatus {
                health: status.to_string(),
                response_time_ms: Some(response_time_ms),
                postgres_version: custom_status.postgres_version.clone(),
                query_result: Some(query_result.clone()),
            };
            if let Ok(status_json) = serde_json::to_string(&status_with_query) {
                ctx.set_custom_status(&status_json);
            }

            ctx.trace_info("[v1.0.3] Query executed, result set in custom status");

            // Carry query result forward so next execution includes it in custom status
            let mut next_input = input.clone();
            next_input.last_query_result = serde_json::to_value(&query_result).ok();
            let input_json = serde_json::to_string(&next_input)
                .map_err(|e| format!("Failed to serialize input: {}", e))?;
            ctx.continue_as_new(input_json).await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(());
        }
    }

    // Step 8: Continue as new (clear last_query_result after one cycle without a query)
    ctx.trace_info("[v1.0.3] Health check cycle complete, restarting instance actor with continue-as-new");
    let mut next_input = input.clone();
    next_input.last_query_result = None;
    let input_json = serde_json::to_string(&next_input)
        .map_err(|e| format!("Failed to serialize input: {}", e))?;
    ctx.continue_as_new(input_json).await
        .map_err(|e| format!("Failed to continue as new: {}", e))?;
    Ok(())
}
