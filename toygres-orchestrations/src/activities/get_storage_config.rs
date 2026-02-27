//! Get storage configuration activity
//!
//! Reads Azure storage config from environment variables.
//! Used by orchestrations to avoid reading env vars directly (non-deterministic).

use duroxide::ActivityContext;
use crate::activity_types::{GetStorageConfigInput, GetStorageConfigOutput};

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::get-storage-config";

pub async fn activity(
    _ctx: ActivityContext,
    _input: GetStorageConfigInput,
) -> Result<GetStorageConfigOutput, String> {
    let storage_account = std::env::var("AZURE_STORAGE_ACCOUNT")
        .unwrap_or_else(|_| "toygresstorage".to_string());
    let container = std::env::var("AZURE_STORAGE_CONTAINER")
        .unwrap_or_else(|_| "toygres-images".to_string());

    Ok(GetStorageConfigOutput {
        storage_account,
        container,
    })
}
