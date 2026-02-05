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

