//! Toygres Orchestrations - Duroxide orchestrations and activities for PostgreSQL management
//! 
//! This crate provides durable workflows (orchestrations) and atomic operations (activities)
//! for managing PostgreSQL instances on Kubernetes.
//! 
//! # Usage
//! 
//! ```rust,no_run
//! use toygres_orchestrations::registry::{create_orchestration_registry, create_activity_registry};
//! use toygres_orchestrations::names::orchestrations;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! let activities = create_activity_registry();
//! let orchestrations = create_orchestration_registry();
//! 
//! // Use with Duroxide runtime
//! // client.start_orchestration(
//! //     "instance-1",
//! //     orchestrations::CREATE_INSTANCE,
//! //     input_json,
//! // ).await?;
//! # Ok(())
//! # }
//! ```

// Orchestration exports
pub mod names;
pub mod types;
pub mod registry;

// Activity exports
pub mod activity_names;
pub mod activity_types;
pub mod k8s_client;

mod orchestrations;
mod activities;

// Re-export key types for convenience
pub use types::*;
pub use activity_types::*;

