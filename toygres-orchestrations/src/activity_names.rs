//! Name constants for Toygres activities
//!
//! Following the Duroxide naming convention: {crate-name}::{type}::{name}

/// Activity names
pub mod activities {
    /// Deploy PostgreSQL to Kubernetes
    /// 
    /// **Input:** [`crate::types::DeployPostgresInput`]  
    /// **Output:** [`crate::types::DeployPostgresOutput`]  
    /// **Idempotent:** Yes (checks if resources exist)
    /// **Operations:**
    /// - Creates PersistentVolumeClaim
    /// - Creates StatefulSet
    /// - Creates Service (LoadBalancer or ClusterIP)
    pub const DEPLOY_POSTGRES: &str = "toygres-activities::activity::deploy-postgres";
    
    /// Delete PostgreSQL deployment from Kubernetes
    /// 
    /// **Input:** [`crate::types::DeletePostgresInput`]  
    /// **Output:** [`crate::types::DeletePostgresOutput`]  
    /// **Idempotent:** Yes (no-op if already deleted)
    /// **Operations:**
    /// - Deletes Service
    /// - Deletes StatefulSet
    /// - Deletes PersistentVolumeClaim
    pub const DELETE_POSTGRES: &str = "toygres-activities::activity::delete-postgres";
    
    /// Wait for PostgreSQL pod to be ready
    /// 
    /// **Input:** [`crate::types::WaitForReadyInput`]  
    /// **Output:** [`crate::types::WaitForReadyOutput`]  
    /// **Idempotent:** Yes (returns immediately if already ready)
    /// **Operations:**
    /// - Polls pod status until Ready condition is True
    /// - Timeout after configured duration
    pub const WAIT_FOR_READY: &str = "toygres-activities::activity::wait-for-ready";
    
    /// Get connection strings for PostgreSQL instance
    /// 
    /// **Input:** [`crate::types::GetConnectionStringsInput`]  
    /// **Output:** [`crate::types::GetConnectionStringsOutput`]  
    /// **Idempotent:** Yes
    /// **Operations:**
    /// - Gets LoadBalancer external IP
    /// - Constructs IP-based connection string
    /// - Constructs DNS-based connection string (if DNS label provided)
    pub const GET_CONNECTION_STRINGS: &str = "toygres-activities::activity::get-connection-strings";
    
    /// Test PostgreSQL connection
    /// 
    /// **Input:** [`crate::types::TestConnectionInput`]  
    /// **Output:** [`crate::types::TestConnectionOutput`]  
    /// **Idempotent:** Yes
    /// **Operations:**
    /// - Connects to PostgreSQL
    /// - Runs SELECT version() query
    /// - Returns version string
    pub const TEST_CONNECTION: &str = "toygres-activities::activity::test-connection";

    /// CMS-related activities
    pub mod cms {
        /// Create CMS record and reserve DNS name
        pub const CREATE_INSTANCE_RECORD: &str = "toygres-orchestrations::activity::cms-create-instance-record";

        /// Update CMS instance state/connection info
        pub const UPDATE_INSTANCE_STATE: &str = "toygres-orchestrations::activity::cms-update-instance-state";

        /// Free DNS name by prefixing with __deleted_
        pub const FREE_DNS_NAME: &str = "toygres-orchestrations::activity::cms-free-dns-name";

        /// Fetch CMS instance by Kubernetes name
        pub const GET_INSTANCE_BY_K8S_NAME: &str = "toygres-orchestrations::activity::cms-get-instance-by-k8s-name";
    }
}

