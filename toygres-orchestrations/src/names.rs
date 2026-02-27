//! Name constants for Toygres orchestrations
//!
//! Re-exports NAME constants from orchestration modules for backward compatibility.
//! Prefer using the NAME constant directly from the orchestration module for better IDE navigation.

/// Orchestration names - re-exported from orchestration modules
pub mod orchestrations {
    pub use crate::orchestrations::create_instance::NAME as CREATE_INSTANCE;
    pub use crate::orchestrations::delete_instance::NAME as DELETE_INSTANCE;
    pub use crate::orchestrations::instance_actor::NAME as INSTANCE_ACTOR;
    pub use crate::orchestrations::system_pruner::NAME as SYSTEM_PRUNER;
    pub use crate::orchestrations::create_image::NAME as CREATE_IMAGE;
    pub use crate::orchestrations::create_semaphore::NAME as CREATE_SEMAPHORE;
    pub use crate::orchestrations::batch_create::NAME as BATCH_CREATE;
}
