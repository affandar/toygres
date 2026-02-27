//! Input and output types for Toygres orchestrations

use serde::{Deserialize, Serialize};
use crate::activity_types::ImageType;

// ============================================================================
// Create Instance Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceInput {
    /// User-friendly instance name (without GUID suffix)
    pub user_name: String,
    /// Instance name
    pub name: String,
    /// PostgreSQL password
    pub password: String,
    /// PostgreSQL version (default: "18")
    pub postgres_version: Option<String>,
    /// Storage size in GB (default: 10)
    pub storage_size_gb: Option<i32>,
    /// Use LoadBalancer for public IP (default: true)
    pub use_load_balancer: Option<bool>,
    /// DNS label for Azure DNS (optional)
    pub dns_label: Option<String>,
    /// Kubernetes namespace (default: "toygres")
    pub namespace: Option<String>,
    /// Unique orchestration/request identifier
    pub orchestration_id: String,
    /// Image type: stock PostgreSQL or pg_durable
    #[serde(default)]
    pub image_type: ImageType,
    /// Source image ID to restore from (optional - if set, creates instance from backup)
    #[serde(default)]
    pub source_image_id: Option<String>,

    /// Optional runtime image ID (toygres_cms.runtime_images.id) used for this deployment.
    /// Stored in CMS for auditability and later delete-protection.
    #[serde(default)]
    pub runtime_image_id: Option<String>,

    /// Optional digest-pinned image pull reference (e.g. toygresacr.azurecr.io/repo@sha256:...)
    /// When provided, deploy activities use this image instead of deriving from image_type.
    #[serde(default)]
    pub image_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceOutput {
    /// Instance name
    pub instance_name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// IP-based connection string
    pub ip_connection_string: String,
    /// DNS-based connection string (if DNS label provided)
    pub dns_connection_string: Option<String>,
    /// External IP address
    pub external_ip: Option<String>,
    /// Azure DNS name
    pub dns_name: Option<String>,
    /// PostgreSQL version
    pub postgres_version: String,
    /// Time taken to deploy (seconds)
    pub deployment_time_seconds: u64,
}

// ============================================================================
// Delete Instance Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteInstanceInput {
    /// Instance name
    pub name: String,
    /// Kubernetes namespace (default: "toygres")
    pub namespace: Option<String>,
    /// Orchestration/request identifier
    pub orchestration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteInstanceOutput {
    /// Instance name
    pub instance_name: String,
    /// Whether instance was deleted (false if didn't exist)
    pub deleted: bool,
}

// ============================================================================
// Instance Actor Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceActorInput {
    /// K8s instance name (with GUID)
    pub k8s_name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// Orchestration ID
    pub orchestration_id: String,
    /// Last query result to carry forward across continue-as-new boundaries
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub last_query_result: Option<serde_json::Value>,
}

// Output: Unit type, continues forever or exits with error
// This orchestration uses continue-as-new and never completes normally

// ============================================================================
// Create Image Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateImageInput {
    /// User-friendly image name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Source instance K8s name (with GUID)
    pub source_k8s_name: String,
    /// Source instance password (optional - fetched from K8s Secret if not provided)
    #[serde(default)]
    pub source_password: Option<String>,
    /// Kubernetes namespace of source instance
    pub namespace: Option<String>,
    /// Unique orchestration/request identifier
    pub orchestration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateImageOutput {
    /// Image name
    pub image_name: String,
    /// Image ID (UUID)
    pub image_id: String,
    /// Blob storage path
    pub blob_path: String,
    /// Backup size in bytes
    pub backup_size_bytes: Option<i64>,
    /// Time taken (seconds)
    pub creation_time_seconds: u64,
}

// ============================================================================
// Create Semaphore Orchestration
// ============================================================================

/// State for the create-semaphore eternal singleton orchestration.
/// Carried across continue-as-new boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemaphoreInput {
    /// Maximum concurrent permits allowed
    pub max_concurrent: u32,
    /// Instance IDs currently holding a permit (BTreeSet for deterministic serialization)
    #[serde(default)]
    pub active: std::collections::BTreeSet<String>,
    /// FIFO queue of instance IDs waiting for a permit
    #[serde(default)]
    pub waiting: std::collections::VecDeque<String>,
    /// Number of events processed in this execution (for continue-as-new threshold)
    #[serde(default)]
    pub events_processed: u32,
}

/// Messages sent to the semaphore's "permits" event queue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PermitMessage {
    /// Request a permit to proceed with creation
    Acquire { requester_id: String },
    /// Release a previously held permit
    Release { requester_id: String },
}

// ============================================================================
// Delete Image Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteImageInput {
    /// Image name
    pub name: String,
    /// Unique orchestration/request identifier
    pub orchestration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteImageOutput {
    /// Image name
    pub image_name: String,
    /// Whether image was deleted
    pub deleted: bool,
}

// ============================================================================
// Batch Create Orchestration
// ============================================================================

/// Input for the batch-create orchestration.
/// Also carries state across continue-as-new boundaries during monitoring phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchCreateInput {
    /// Base name for instances (e.g. "mydb" → mydb1, mydb2, ...)
    pub base_name: String,
    /// Number of instances to create
    pub count: u32,
    /// PostgreSQL password
    pub password: String,
    /// PostgreSQL version (e.g. "18")
    #[serde(default = "default_postgres_version")]
    pub postgres_version: String,
    /// Storage size in GB
    #[serde(default = "default_storage_size")]
    pub storage_size_gb: i32,
    /// Whether to use LoadBalancer for public IP
    #[serde(default = "default_true")]
    pub use_load_balancer: bool,
    /// Kubernetes namespace
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// Image type
    #[serde(default)]
    pub image_type: ImageType,
    /// Optional runtime image ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_image_id: Option<String>,
    /// Optional image override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_override: Option<String>,
    /// Instances being tracked (populated after creation phase)
    #[serde(default)]
    pub instances: Vec<BatchInstance>,
    /// Number of polls completed (for continue-as-new threshold)
    #[serde(default)]
    pub polls_completed: u32,
    /// Accumulated errors from failed children
    #[serde(default)]
    pub errors: Vec<BatchError>,
}

fn default_postgres_version() -> String { "18".to_string() }
fn default_storage_size() -> i32 { 10 }
fn default_true() -> bool { true }
fn default_namespace() -> String { "toygres".to_string() }

/// An individual instance in a batch
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchInstance {
    pub user_name: String,
    pub k8s_name: String,
    pub orchestration_id: String,
}

/// Error from a failed child create
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchError {
    pub instance_name: String,
    pub error: String,
}

/// Output from the batch-create orchestration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchCreateOutput {
    pub total: u32,
    pub completed: u32,
    pub failed: u32,
    pub errors: Vec<BatchError>,
}

