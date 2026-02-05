//! Input and output types for Toygres activities

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Image Types
// ============================================================================

/// PostgreSQL image type for deployment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageType {
    /// Stock PostgreSQL image (postgres:version)
    #[default]
    Stock,
    /// pg_durable image with duroxide extension
    PgDurable,
}

impl ImageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageType::Stock => "stock",
            ImageType::PgDurable => "pg_durable",
        }
    }
    
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pg_durable" | "pgdurable" | "durable" => ImageType::PgDurable,
            _ => ImageType::Stock,
        }
    }
}

// ============================================================================
// Deploy PostgreSQL Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployPostgresInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name (used for K8s resource names)
    pub instance_name: String,
    /// PostgreSQL password
    pub password: String,
    /// PostgreSQL version (e.g., "16", "18")
    pub postgres_version: String,
    /// Storage size in GB
    pub storage_size_gb: i32,
    /// Use LoadBalancer (true) or ClusterIP (false)
    pub use_load_balancer: bool,
    /// Optional DNS label for Azure DNS
    pub dns_label: Option<String>,
    /// Image type (stock or pg_durable)
    #[serde(default)]
    pub image_type: ImageType,
    /// Custom image registry (for pg_durable images)
    pub image_registry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployPostgresOutput {
    /// Instance name
    pub instance_name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// Whether resources were created (false if already existed)
    pub created: bool,
}

// ============================================================================
// Deploy PostgreSQL Activity (v2 - supports image override)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployPostgresV2Input {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name (used for K8s resource names)
    pub instance_name: String,
    /// PostgreSQL password
    pub password: String,
    /// PostgreSQL version (e.g., "16", "18")
    pub postgres_version: String,
    /// Storage size in GB
    pub storage_size_gb: i32,
    /// Use LoadBalancer (true) or ClusterIP (false)
    pub use_load_balancer: bool,
    /// Optional DNS label for Azure DNS
    pub dns_label: Option<String>,
    /// Image type (stock or pg_durable)
    #[serde(default)]
    pub image_type: ImageType,
    /// Custom image registry (for pg_durable images)
    pub image_registry: Option<String>,
    /// Optional fully-qualified, digest-pinned image pull reference.
    /// When set, takes precedence over image_type/image_registry.
    #[serde(default)]
    pub image_override: Option<String>,
}

// ============================================================================
// Delete PostgreSQL Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeletePostgresInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeletePostgresOutput {
    /// Whether resources were deleted (false if didn't exist)
    pub deleted: bool,
}

// ============================================================================
// Wait For Ready Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaitForReadyInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
    /// Timeout in seconds (0 = no timeout, just check current status)
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaitForReadyOutput {
    /// Pod phase (e.g., "Running", "Pending")
    pub pod_phase: String,
    /// Whether pod is ready
    pub is_ready: bool,
}

// ============================================================================
// Get Connection Strings Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetConnectionStringsInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
    /// PostgreSQL password
    pub password: String,
    /// Whether LoadBalancer was used
    pub use_load_balancer: bool,
    /// DNS label (if used)
    pub dns_label: Option<String>,
    /// Image type (affects default database name)
    #[serde(default)]
    pub image_type: ImageType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetConnectionStringsOutput {
    /// IP-based connection string (external, for clients outside the cluster)
    pub ip_connection_string: String,
    /// DNS-based connection string (if DNS label provided)
    pub dns_connection_string: Option<String>,
    /// Internal connection string (ClusterIP-based, for use within the cluster)
    pub internal_connection_string: String,
    /// External IP address (if LoadBalancer)
    pub external_ip: Option<String>,
    /// Azure DNS name (if DNS label provided)
    pub dns_name: Option<String>,
}

// ============================================================================
// Test Connection Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestConnectionInput {
    /// Connection string to test
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestConnectionOutput {
    /// PostgreSQL version string
    pub version: String,
    /// Whether connection succeeded
    pub connected: bool,
}

// ============================================================================
// Set Password Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPasswordInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
    /// New password to set
    pub new_password: String,
    /// Internal connection string (ClusterIP-based, accessible from within the cluster)
    /// This should use the current/default password to connect
    pub internal_connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPasswordOutput {
    /// Whether password was set successfully
    pub success: bool,
}

// ============================================================================
// CMS Activities
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceRecordInput {
    pub user_name: String,
    pub k8s_name: String,
    pub namespace: String,
    pub postgres_version: String,
    pub storage_size_gb: i32,
    pub use_load_balancer: bool,
    pub dns_name: Option<String>,
    pub orchestration_id: String,
    #[serde(default)]
    pub image_type: ImageType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceRecordOutput {
    pub instance_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInstanceStateInput {
    pub k8s_name: String,
    pub state: String,
    pub ip_connection_string: Option<String>,
    pub dns_connection_string: Option<String>,
    pub external_ip: Option<String>,
    pub delete_orchestration_id: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInstanceStateOutput {
    pub updated: bool,
    pub previous_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetInstanceRuntimeImageInput {
    pub k8s_name: String,
    pub runtime_image_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetInstanceRuntimeImageOutput {
    pub updated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeDnsNameInput {
    pub k8s_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeDnsNameOutput {
    pub freed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetInstanceByK8sNameInput {
    pub k8s_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CmsInstanceRecord {
    pub id: Uuid,
    pub user_name: String,
    pub k8s_name: String,
    pub namespace: String,
    pub state: String,
    pub dns_name: Option<String>,
    pub postgres_version: String,
    pub storage_size_gb: i32,
    pub image_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetInstanceByK8sNameOutput {
    pub found: bool,
    pub record: Option<CmsInstanceRecord>,
    pub instance_actor_orchestration_id: Option<String>,
}

// ============================================================================
// Get Instance Connection Activity (CMS)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetInstanceConnectionInput {
    pub k8s_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetInstanceConnectionOutput {
    pub found: bool,
    pub connection_string: Option<String>,
    pub state: Option<String>,
}

// ============================================================================
// Record Health Check Activity (CMS)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordHealthCheckInput {
    pub k8s_name: String,
    pub status: String,  // "healthy", "unhealthy", "unknown"
    pub postgres_version: Option<String>,
    pub response_time_ms: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordHealthCheckOutput {
    pub recorded: bool,
    pub check_id: i64,
}

// ============================================================================
// Update Instance Health Activity (CMS)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInstanceHealthInput {
    pub k8s_name: String,
    pub health_status: String,  // "healthy", "unhealthy", "unknown"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInstanceHealthOutput {
    pub updated: bool,
}

// ============================================================================
// Record Instance Actor Activity (CMS)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordInstanceActorInput {
    pub k8s_name: String,
    pub instance_actor_orchestration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordInstanceActorOutput {
    pub recorded: bool,
}

// ============================================================================
// Delete Instance Record Activity (CMS)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteInstanceRecordInput {
    pub k8s_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteInstanceRecordOutput {
    pub deleted: bool,
}

// ============================================================================
// Raise Event Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaiseEventInput {
    /// Target orchestration instance ID
    pub instance_id: String,
    /// Event name
    pub event_name: String,
    /// Event data (JSON string)
    pub event_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaiseEventOutput {
    /// Whether the event was raised successfully
    pub raised: bool,
}

// ============================================================================
// System Prune Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemPruneInput {
    /// Unique run identifier for this pruner instance
    pub run_id: String,
    /// Current iteration count
    pub iteration: u64,
    /// Delete terminal instances older than N hours (default: 6)
    #[serde(default = "default_delete_hours")]
    pub delete_terminal_older_than_hours: u64,
    /// Keep only the last N executions per instance (default: 1 = current only)
    #[serde(default = "default_keep_executions")]
    pub keep_executions: u32,
}

fn default_delete_hours() -> u64 {
    6
}

fn default_keep_executions() -> u32 {
    1
}

impl Default for SystemPruneInput {
    fn default() -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            iteration: 1,
            delete_terminal_older_than_hours: default_delete_hours(),
            keep_executions: default_keep_executions(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SystemPruneOutput {
    /// Current iteration number
    pub iteration: u64,
    /// Number of terminal instances deleted
    pub instances_deleted: u64,
    /// Number of executions pruned
    pub executions_pruned: u64,
    /// Number of instances that had executions pruned
    pub instances_pruned: u64,
    /// Detailed log of what was pruned/deleted
    pub prune_log: Vec<PruneLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PruneLogEntry {
    /// Timestamp of the operation
    pub timestamp: String,
    /// Type of operation: "delete" or "prune"
    pub operation: String,
    /// Instance ID affected
    pub instance_id: String,
    /// Orchestration name
    pub orchestration_name: String,
    /// Status before operation
    pub status: String,
    /// Additional details
    pub details: String,
}

// ============================================================================
// Image State Enum
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ImageState {
    Creating,
    Ready,
    Failed,
    Deleting,
    Deleted,
}

impl ImageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageState::Creating => "creating",
            ImageState::Ready => "ready",
            ImageState::Failed => "failed",
            ImageState::Deleting => "deleting",
            ImageState::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "creating" => ImageState::Creating,
            "ready" => ImageState::Ready,
            "failed" => ImageState::Failed,
            "deleting" => ImageState::Deleting,
            "deleted" => ImageState::Deleted,
            _ => ImageState::Creating,
        }
    }
}

// ============================================================================
// Image CMS Operations (Consolidated Activity)
// ============================================================================

/// Enum-based CMS operations for images
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ImageOperation {
    /// Create a new image record (idempotent via orchestration_id)
    Create {
        name: String,
        description: Option<String>,
        source_instance_id: Uuid,
        source_k8s_name: String,
        source_namespace: String,
        blob_storage_url: String,
        blob_container: String,
        blob_path: String,
        storage_size_gb: i32,
        postgres_version: String,
        image_type: String,
        source_password_encrypted: Vec<u8>,
        orchestration_id: String,
    },
    
    /// Update image state (creating → ready, failed, deleting, deleted)
    UpdateState {
        name: String,
        state: ImageState,
        backup_size_bytes: Option<i64>,
        backup_checksum: Option<String>,
        error_message: Option<String>,
    },
    
    /// Get image by name
    Get { name: String },
    
    /// Get image by ID
    GetById { id: Uuid },
    
    /// List all active images
    List,
    
    /// Mark image as deleted (soft delete)
    Delete { name: String },

    /// Get source password for restore (decrypts the stored password)
    GetSourcePassword { id: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRecord {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub source_instance_id: Option<Uuid>,
    pub source_k8s_name: String,
    pub source_namespace: String,
    pub blob_storage_url: String,
    pub blob_container: String,
    pub blob_path: String,
    pub storage_size_gb: i32,
    pub postgres_version: String,
    pub image_type: String,
    pub backup_size_bytes: Option<i64>,
    pub backup_checksum: Option<String>,
    pub state: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub ready_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ImageOperationResult {
    Created { image_id: Uuid },
    Updated { updated: bool },
    Found { record: ImageRecord },
    NotFound,
    Listed { images: Vec<ImageRecord> },
    Deleted { deleted: bool },
    PasswordFound { password: String },
}

// ============================================================================
// Run Backup Job Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBackupJobInput {
    pub job_name: String,
    pub namespace: String,
    pub source_instance_name: String,
    pub source_password: String,
    pub blob_storage_account: String,
    pub blob_container: String,
    pub blob_path: String,
    pub image_name: String,
    pub postgres_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBackupJobOutput {
    pub job_name: String,
    pub created: bool,
}

// ============================================================================
// Wait For Job Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForJobInput {
    pub job_name: String,
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForJobOutput {
    pub succeeded: bool,
    pub failed: bool,
    pub active: bool,
    pub completion_time: Option<String>,
    pub message: Option<String>,
}

// ============================================================================
// Delete Job Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteJobInput {
    pub job_name: String,
    pub namespace: String,
    pub delete_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteJobOutput {
    pub deleted: bool,
}

// ============================================================================
// Delete Blob Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBlobInput {
    pub blob_storage_account: String,
    pub blob_container: String,
    pub blob_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBlobOutput {
    pub deleted: bool,
}

// ============================================================================
// Get Instance Password Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInstancePasswordInput {
    /// K8s instance name (used to construct secret name: {k8s_name}-secret)
    pub k8s_name: String,
    /// Kubernetes namespace
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInstancePasswordOutput {
    /// Whether the secret was found
    pub found: bool,
    /// The password if found
    pub password: Option<String>,
}

// ============================================================================
// Send External Event Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendExternalEventInput {
    /// Target orchestration instance ID
    pub instance_id: String,
    /// Event name (e.g., "InstanceDeleted")
    pub event_name: String,
    /// Event payload (JSON string)
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendExternalEventOutput {
    /// Whether the event was sent successfully
    pub sent: bool,
}

// ============================================================================
// Create PVC Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePvcInput {
    /// Name of the PVC (typically same as instance name)
    pub name: String,
    /// Namespace for the PVC
    pub namespace: String,
    /// Storage size in GB
    pub storage_size_gb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePvcOutput {
    /// Name of the created PVC
    pub pvc_name: String,
    /// Whether the PVC was created (false if it already existed)
    pub created: bool,
}

