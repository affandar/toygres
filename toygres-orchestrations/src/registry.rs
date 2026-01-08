//! Registry builders for Toygres orchestrations and activities

use duroxide::runtime::registry::ActivityRegistry;
use duroxide::OrchestrationRegistry;
use crate::names::orchestrations;
use crate::activities;

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
        // v1.0.1: CMS updates now propagate errors (schema-qualified enums)
        .register_versioned_typed(
            orchestrations::CREATE_INSTANCE,
            "1.0.1",
            crate::orchestrations::create_instance::create_instance_1_0_1,
        )
        .register_typed(
            orchestrations::DELETE_INSTANCE,
            crate::orchestrations::delete_instance::delete_instance_orchestration,
        )
        // v1.0.1: CMS updates now propagate errors (schema-qualified enums)
        .register_versioned_typed(
            orchestrations::DELETE_INSTANCE,
            "1.0.1",
            crate::orchestrations::delete_instance::delete_instance_1_0_1,
        )
        .register_typed(
            orchestrations::INSTANCE_ACTOR,
            crate::orchestrations::instance_actor::instance_actor_orchestration,
        )
        .register_typed(
            orchestrations::SYSTEM_PRUNER,
            crate::orchestrations::system_pruner::system_pruner_orchestration,
        )
        // Register v1.0.1 of system pruner (2 min timer, keep 2 executions)
        .register_versioned_typed(
            orchestrations::SYSTEM_PRUNER,
            "1.0.1",
            crate::orchestrations::system_pruner::system_pruner_1_0_1,
        )
        // Register v1.0.2 of system pruner (5 min timer, keep 2 executions)
        .register_versioned_typed(
            orchestrations::SYSTEM_PRUNER,
            "1.0.2",
            crate::orchestrations::system_pruner::system_pruner_1_0_2,
        )
        // Register v1.0.3 of system pruner (uses system-prune-2, keep 3 executions)
        .register_versioned_typed(
            orchestrations::SYSTEM_PRUNER,
            "1.0.3",
            crate::orchestrations::system_pruner::system_pruner_1_0_3,
        )
        // Register v1.0.4 of system pruner (1 min timer, uses system-prune-2, keep 3 executions)
        .register_versioned_typed(
            orchestrations::SYSTEM_PRUNER,
            "1.0.4",
            crate::orchestrations::system_pruner::system_pruner_1_0_4,
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
        // K8s activities
        .register_typed(
            activities::deploy_postgres::NAME,
            activities::deploy_postgres::activity,
        )
        .register_typed(
            activities::delete_postgres::NAME,
            activities::delete_postgres::activity,
        )
        .register_typed(
            activities::wait_for_ready::NAME,
            activities::wait_for_ready::activity,
        )
        .register_typed(
            activities::get_connection_strings::NAME,
            activities::get_connection_strings::activity,
        )
        .register_typed(
            activities::test_connection::NAME,
            activities::test_connection::activity,
        )
        .register_typed(
            activities::set_password::NAME,
            activities::set_password::activity,
        )
        // CMS activities
        .register_typed(
            activities::cms::create_instance_record::NAME,
            activities::cms::create_instance_record::activity,
        )
        .register_typed(
            activities::cms::update_instance_state::NAME,
            activities::cms::update_instance_state::activity,
        )
        .register_typed(
            activities::cms::free_dns_name::NAME,
            activities::cms::free_dns_name::activity,
        )
        .register_typed(
            activities::cms::get_instance_by_k8s_name::NAME,
            activities::cms::get_instance_by_k8s_name::activity,
        )
        .register_typed(
            activities::cms::get_instance_connection::NAME,
            activities::cms::get_instance_connection::activity,
        )
        .register_typed(
            activities::cms::record_health_check::NAME,
            activities::cms::record_health_check::activity,
        )
        .register_typed(
            activities::cms::update_instance_health::NAME,
            activities::cms::update_instance_health::activity,
        )
        .register_typed(
            activities::cms::record_instance_actor::NAME,
            activities::cms::record_instance_actor::activity,
        )
        .register_typed(
            activities::cms::delete_instance_record::NAME,
            activities::cms::delete_instance_record::activity,
        )
        // System activities
        .register_typed(
            activities::system_prune::NAME,
            activities::system_prune::activity,
        )
        .register_typed(
            activities::system_prune_2::NAME,
            activities::system_prune_2::activity,
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_orchestration_registry_can_be_created() {
        let registry = create_orchestration_registry();
        // Verify system pruner is registered
        assert!(
            registry.resolve_handler(crate::names::orchestrations::SYSTEM_PRUNER).is_some(),
            "SYSTEM_PRUNER should be registered in the orchestration registry"
        );
    }
    
    #[test]
    fn test_activity_registry_can_be_created() {
        let _registry = create_activity_registry();
        // Registry creation should not panic
    }
}

