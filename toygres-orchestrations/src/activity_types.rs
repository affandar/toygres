//! Input and output types for Toygres activities

use serde::{Deserialize, Serialize};

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

