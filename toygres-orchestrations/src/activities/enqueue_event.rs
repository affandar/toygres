/// Enqueue Event Activity
///
/// Enqueues a message onto another orchestration instance's named event queue.
/// Used by create-instance orchestrations to send Acquire/Release messages
/// to the create-semaphore singleton.

use duroxide::ActivityContext;

use crate::activity_types::{EnqueueEventInput, EnqueueEventOutput};

pub const NAME: &str = "toygres-orchestrations::activity::enqueue-event";

pub async fn activity(
    ctx: ActivityContext,
    input: EnqueueEventInput,
) -> Result<EnqueueEventOutput, String> {
    ctx.trace_info(format!(
        "Enqueuing event to instance '{}' queue '{}'",
        input.instance_id, input.queue_name
    ));

    let client = ctx.get_client();

    match client
        .enqueue_event(&input.instance_id, &input.queue_name, &input.payload)
        .await
    {
        Ok(_) => {
            ctx.trace_info(format!(
                "Successfully enqueued event to '{}' queue '{}'",
                input.instance_id, input.queue_name
            ));
            Ok(EnqueueEventOutput { sent: true })
        }
        Err(e) => {
            ctx.trace_warn(format!(
                "Failed to enqueue event to '{}' queue '{}': {}",
                input.instance_id, input.queue_name, e
            ));
            // Return error so orchestration can handle retry
            Err(format!("Failed to enqueue event: {}", e))
        }
    }
}
