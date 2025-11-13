//! Input and output types for Toygres orchestrations

use serde::{Deserialize, Serialize};

// ============================================================================
// Create Instance Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceInput {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteInstanceOutput {
    /// Instance name
    pub instance_name: String,
    /// Whether instance was deleted (false if didn't exist)
    pub deleted: bool,
}

