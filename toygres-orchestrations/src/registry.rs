//! Registry builders for Toygres orchestrations and activities

use duroxide::runtime::registry::ActivityRegistry;
use duroxide::OrchestrationRegistry;
use crate::activity_names::activities;
use crate::names::orchestrations;

/// Create an OrchestrationRegistry with all Toygres orchestrations
///
/// # Example
///
/// ```rust,no_run
/// use toygres_orchestrations::registry::create_orchestration_registry;
/// 
/// let orchestrations = create_orchestration_registry();
/// ```
pub fn create_orchestration_registry() -> OrchestrationRegistry {
    OrchestrationRegistry::builder()
        .register_typed(
            orchestrations::CREATE_INSTANCE,
            crate::orchestrations::create_instance::create_instance_orchestration,
        )
        .register_typed(
            orchestrations::DELETE_INSTANCE,
            crate::orchestrations::delete_instance::delete_instance_orchestration,
        )
        .build()
}

/// Create an ActivityRegistry with all Toygres activities
///
/// # Example
///
/// ```rust,no_run
/// use toygres_orchestrations::registry::create_activity_registry;
/// 
/// let activities = create_activity_registry();
/// ```
pub fn create_activity_registry() -> ActivityRegistry {
    ActivityRegistry::builder()
        .register_typed(
            activities::DEPLOY_POSTGRES,
            crate::activities::deploy_postgres::deploy_postgres_activity,
        )
        .register_typed(
            activities::DELETE_POSTGRES,
            crate::activities::delete_postgres::delete_postgres_activity,
        )
        .register_typed(
            activities::WAIT_FOR_READY,
            crate::activities::wait_for_ready::wait_for_ready_activity,
        )
        .register_typed(
            activities::GET_CONNECTION_STRINGS,
            crate::activities::get_connection_strings::get_connection_strings_activity,
        )
        .register_typed(
            activities::TEST_CONNECTION,
            crate::activities::test_connection::test_connection_activity,
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_orchestration_registry_can_be_created() {
        let _registry = create_orchestration_registry();
        // Registry creation should not panic
    }
    
    #[test]
    fn test_activity_registry_can_be_created() {
        let _registry = create_activity_registry();
        // Registry creation should not panic
    }
}

