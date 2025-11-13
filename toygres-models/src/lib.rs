use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents the state of a PostgreSQL instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "instance_state", rename_all = "lowercase")]
pub enum InstanceState {
    Creating,
    Running,
    Deleting,
    Deleted,
    Failed,
}

/// Represents the health status of a PostgreSQL instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "health_status", rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Metadata about a PostgreSQL instance
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InstanceMetadata {
    pub id: Uuid,
    pub name: String,
    pub state: InstanceState,
    pub health_status: HealthStatus,
    pub connection_string: Option<String>,
    pub health_check_orchestration_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Configuration for deploying a new PostgreSQL instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub name: String,
    pub username: String,
    pub password: String,
    pub storage_size_gb: i32,
    pub postgres_version: String,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            username: "postgres".to_string(),
            password: String::new(),
            storage_size_gb: 10,
            postgres_version: "16".to_string(),
        }
    }
}

/// Request to create a new PostgreSQL instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub storage_size_gb: Option<i32>,
    #[serde(default)]
    pub postgres_version: Option<String>,
}

/// Response from creating a PostgreSQL instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceResponse {
    pub instance_id: Uuid,
    pub connection_string: String,
    pub orchestration_id: String,
}

/// Response from listing instances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListInstancesResponse {
    pub instances: Vec<InstanceMetadata>,
}

/// Response for operation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStatus {
    pub orchestration_id: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

