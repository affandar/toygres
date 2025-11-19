//! Input and output types for Toygres activities

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetConnectionStringsOutput {
    /// IP-based connection string
    pub ip_connection_string: String,
    /// DNS-based connection string (if DNS label provided)
    pub dns_connection_string: Option<String>,
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

