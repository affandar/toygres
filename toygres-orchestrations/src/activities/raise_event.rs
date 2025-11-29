//! Raise event to another orchestration activity

use duroxide::ActivityContext;
use crate::activity_types::{RaiseEventInput, RaiseEventOutput};
use once_cell::sync::OnceCell;
use std::sync::Arc;
use duroxide::Client;

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::raise-event";

static DUROXIDE_CLIENT: OnceCell<Arc<Client>> = OnceCell::new();

/// Initialize the duroxide client for use in activities
pub fn init_client(client: Arc<Client>) {
    DUROXIDE_CLIENT.set(client).ok();
}

/// Get the duroxide client
fn get_client() -> Result<Arc<Client>, String> {
    DUROXIDE_CLIENT
        .get()
        .cloned()
        .ok_or_else(|| "Duroxide client not initialized".to_string())
}

pub async fn activity(
    ctx: ActivityContext,
    input: RaiseEventInput,
) -> Result<RaiseEventOutput, String> {
    ctx.trace_info(format!(
        "Raising event '{}' to orchestration '{}'",
        input.event_name, input.instance_id
    ));
    
    let client = get_client()?;
    
    client
        .raise_event(&input.instance_id, &input.event_name, &input.event_data)
        .await
        .map_err(|e| format!("Failed to raise event: {}", e))?;
    
    ctx.trace_info(format!(
        "Event '{}' raised successfully to '{}'",
        input.event_name, input.instance_id
    ));
    
    Ok(RaiseEventOutput { raised: true })
}


