/// Create Semaphore Orchestration v1.0.0
///
/// Eternal singleton that throttles concurrent create-instance orchestrations.
///
/// State (carried across continue-as-new):
/// - active: BTreeSet<String> — instance IDs holding permits
/// - waiting: VecDeque<String> — FIFO queue of instance IDs waiting
/// - max_concurrent: u32 — max simultaneous creates (default 10)
/// - events_processed: u32 — counter for continue-as-new threshold
///
/// Event queue "permits" receives PermitMessage::{Acquire, Release}
/// Grants via send-external-event activity → raise_event(requester, "create-permit", "granted")

use duroxide::OrchestrationContext;

use crate::activities;
use crate::activity_types::{SendExternalEventInput, SendExternalEventOutput};
use crate::types::{SemaphoreInput, PermitMessage};

const PERMITS_QUEUE: &str = "permits";
const PERMIT_EVENT_NAME: &str = "create-permit";
const CONTINUE_AS_NEW_THRESHOLD: u32 = 50;

pub async fn create_semaphore_1_0_0_orchestration(
    ctx: OrchestrationContext,
    mut input: SemaphoreInput,
) -> Result<(), String> {
    ctx.trace_info(format!(
        "[semaphore v1.0.0] Starting. active: {}/{}, waiting: {}, events_processed: {}",
        input.active.len(),
        input.max_concurrent,
        input.waiting.len(),
        input.events_processed,
    ));

    ctx.set_custom_status(&format!(
        "active: {}/{}, waiting: {}",
        input.active.len(),
        input.max_concurrent,
        input.waiting.len(),
    ));

    // Main event loop — process messages until continue-as-new threshold
    loop {
        // Dequeue next message from "permits" queue (blocks until one arrives)
        let msg: PermitMessage = ctx
            .dequeue_event_typed(PERMITS_QUEUE)
            .await;

        match msg {
            PermitMessage::Acquire { requester_id } => {
                ctx.trace_info(format!(
                    "[semaphore v1.0.0] Acquire from '{}' (active: {}/{})",
                    requester_id,
                    input.active.len(),
                    input.max_concurrent,
                ));

                if (input.active.len() as u32) < input.max_concurrent {
                    // Grant immediately
                    input.active.insert(requester_id.clone());
                    grant_permit(&ctx, &requester_id).await;
                } else {
                    // Queue it
                    input.waiting.push_back(requester_id);
                }
            }
            PermitMessage::Release { requester_id } => {
                ctx.trace_info(format!(
                    "[semaphore v1.0.0] Release from '{}' (active: {}/{})",
                    requester_id,
                    input.active.len(),
                    input.max_concurrent,
                ));

                input.active.remove(&requester_id);

                // Grant to next waiter if any
                if let Some(next) = input.waiting.pop_front() {
                    input.active.insert(next.clone());
                    grant_permit(&ctx, &next).await;
                }
            }
        }

        input.events_processed += 1;

        // Update custom status
        ctx.set_custom_status(&format!(
            "active: {}/{}, waiting: {}, processed: {}",
            input.active.len(),
            input.max_concurrent,
            input.waiting.len(),
            input.events_processed,
        ));

        // Continue-as-new to bound history growth
        if input.events_processed >= CONTINUE_AS_NEW_THRESHOLD {
            ctx.trace_info(format!(
                "[semaphore v1.0.0] Continue-as-new after {} events (active: {}, waiting: {})",
                input.events_processed,
                input.active.len(),
                input.waiting.len(),
            ));

            // Reset counter for next execution
            input.events_processed = 0;

            let input_json = serde_json::to_string(&input)
                .map_err(|e| format!("Failed to serialize semaphore state: {}", e))?;
            ctx.continue_as_new(input_json)
                .await
                .map_err(|e| format!("Failed to continue as new: {}", e))?;
            return Ok(());
        }
    }
}

/// Grant a permit to a requester by sending a named event
async fn grant_permit(ctx: &OrchestrationContext, requester_id: &str) {
    ctx.trace_info(format!(
        "[semaphore v1.0.0] Granting permit to '{}'",
        requester_id,
    ));

    let send_input = SendExternalEventInput {
        instance_id: requester_id.to_string(),
        event_name: PERMIT_EVENT_NAME.to_string(),
        payload: "granted".to_string(),
    };

    match ctx
        .schedule_activity_typed::<SendExternalEventInput, SendExternalEventOutput>(
            activities::send_external_event::NAME,
            &send_input,
        )
        .await
    {
        Ok(output) => {
            if !output.sent {
                ctx.trace_warn(format!(
                    "[semaphore v1.0.0] Failed to send permit to '{}' (target may have completed)",
                    requester_id,
                ));
            }
        }
        Err(e) => {
            ctx.trace_warn(format!(
                "[semaphore v1.0.0] Error sending permit to '{}': {}",
                requester_id, e,
            ));
        }
    }
}
