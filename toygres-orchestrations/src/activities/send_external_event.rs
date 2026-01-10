/// Send External Event Activity
///
/// Sends an external event to another orchestration instance.
/// Used to signal orchestrations (like instance actors) from other orchestrations.

use duroxide::ActivityContext;

use crate::activity_types::{SendExternalEventInput, SendExternalEventOutput};

pub const NAME: &str = "toygres-orchestrations::activity::send-external-event";

pub async fn activity(
    ctx: ActivityContext,
    input: SendExternalEventInput,
) -> Result<SendExternalEventOutput, String> {
    ctx.trace_info(format!(
        "Sending external event '{}' to instance '{}'",
        input.event_name, input.instance_id
    ));

    let client = ctx.get_client();

    // TODO: Replace with ctx.raise_external_event() when duroxide adds it as a system function
    // See: https://github.com/affandar/duroxide/issues/52
    match client
        .raise_event(&input.instance_id, &input.event_name, &input.payload)
        .await
    {
        Ok(_) => {
            ctx.trace_info(format!(
                "Successfully sent '{}' event to '{}'",
                input.event_name, input.instance_id
            ));
            Ok(SendExternalEventOutput { sent: true })
        }
        Err(e) => {
            // Log warning but don't fail - the target instance might already be completed
            ctx.trace_warn(format!(
                "Failed to send '{}' event to '{}': {}",
                input.event_name, input.instance_id, e
            ));
            Ok(SendExternalEventOutput { sent: false })
        }
    }
}
