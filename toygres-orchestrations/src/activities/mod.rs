pub mod deploy_postgres;
pub mod deploy_postgres_from_pvc;
pub mod delete_postgres;
pub mod wait_for_ready;
pub mod get_connection_strings;
pub mod test_connection;
pub mod set_password;
pub mod cms;
pub mod system_prune;
pub mod system_prune_2;
pub mod run_backup_job;
pub mod run_restore_job;
pub mod send_external_event;

// Consolidated K8s utility activities
pub mod k8s_utils;

// Backward-compatible re-exports for the consolidated modules
// These allow existing code to continue using the old module paths
pub mod create_pvc {
    pub use super::k8s_utils::names::CREATE_PVC as NAME;
    pub use super::k8s_utils::create_pvc as activity;
    pub use crate::activity_types::{CreatePvcInput, CreatePvcOutput};
}

pub mod delete_job {
    pub use super::k8s_utils::names::DELETE_JOB as NAME;
    pub use super::k8s_utils::delete_job as activity;
    pub use crate::activity_types::{DeleteJobInput, DeleteJobOutput};
}

pub mod wait_for_job {
    pub use super::k8s_utils::names::WAIT_FOR_JOB as NAME;
    pub use super::k8s_utils::wait_for_job as activity;
    pub use crate::activity_types::{WaitForJobInput, WaitForJobOutput};
}

pub mod get_instance_password {
    pub use super::k8s_utils::names::GET_INSTANCE_PASSWORD as NAME;
    pub use super::k8s_utils::get_instance_password as activity;
    pub use crate::activity_types::{GetInstancePasswordInput, GetInstancePasswordOutput};
}
