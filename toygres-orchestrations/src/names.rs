//! Name constants for Toygres orchestrations
//!
//! Following the Duroxide naming convention: {crate-name}::{type}::{name}

/// Orchestration names
pub mod orchestrations {
    /// Create a new PostgreSQL instance
    /// 
    /// **Input:** [`crate::types::CreateInstanceInput`]  
    /// **Output:** [`crate::types::CreateInstanceOutput`]  
    /// **Activities used:**
    /// - [`toygres_activities::names::activities::DEPLOY_POSTGRES`]
    /// - [`toygres_activities::names::activities::WAIT_FOR_READY`]
    /// - [`toygres_activities::names::activities::GET_CONNECTION_STRINGS`]
    /// - [`toygres_activities::names::activities::TEST_CONNECTION`]
    /// **Duration:** ~30-60 seconds
    pub const CREATE_INSTANCE: &str = "toygres-orchestrations::orchestration::create-instance";
    
    /// Delete a PostgreSQL instance
    /// 
    /// **Input:** [`crate::types::DeleteInstanceInput`]  
    /// **Output:** [`crate::types::DeleteInstanceOutput`]  
    /// **Activities used:**
    /// - [`toygres_activities::names::activities::DELETE_POSTGRES`]
    /// **Duration:** ~10 seconds
    /// **Note:** In Phase 3, will also cancel health check orchestration
    pub const DELETE_INSTANCE: &str = "toygres-orchestrations::orchestration::delete-instance";
    
    // Phase 3: Health check orchestration
    // pub const HEALTH_CHECK: &str = "toygres-orchestrations::orchestration::health-check";
}

