//! Registry builders for Toygres orchestrations and activities

use duroxide::runtime::registry::ActivityRegistry;
use duroxide::OrchestrationRegistry;
use crate::activities;
use crate::orchestrations::{create_instance, delete_instance, instance_actor, system_pruner, create_image, create_image_v1_0_1};

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
        // v1.0.5: Unified orchestration with inlined restore logic
        .register_versioned_typed(
            create_instance::NAME,
            "1.0.5",
            create_instance::create_instance_1_0_5_orchestration,
        )
        // v1.0.6: Doubled pod ready timeout (60 → 120 attempts / 5min → 10min)
        .register_versioned_typed(
            create_instance::NAME,
            "1.0.6",
            create_instance::create_instance_1_0_6_orchestration,
        )
        // v1.0.7: Custom status reporting at every step
        .register_versioned_typed(
            create_instance::NAME,
            "1.0.7",
            create_instance::create_instance_1_0_7_orchestration,
        )
        .register_typed(
            delete_instance::NAME,
            delete_instance::delete_instance_1_0_0_orchestration,
        )
        // v1.0.1: CMS updates now propagate errors (schema-qualified enums)
        .register_versioned_typed(
            delete_instance::NAME,
            "1.0.1",
            delete_instance::delete_instance_1_0_1_orchestration,
        )
        // v1.0.2: Send InstanceDeleted signal to actor before cleanup
        .register_versioned_typed(
            delete_instance::NAME,
            "1.0.2",
            delete_instance::delete_instance_1_0_2_orchestration,
        )
        // v1.0.3: Custom status reporting
        .register_versioned_typed(
            delete_instance::NAME,
            "1.0.3",
            delete_instance::delete_instance_1_0_3_orchestration,
        )
        .register_typed(
            instance_actor::NAME,
            instance_actor::instance_actor_1_0_0_orchestration,
        )
        .register_versioned_typed(
            instance_actor::NAME,
            "1.0.1",
            instance_actor::instance_actor_1_0_1_orchestration,
        )
        .register_versioned_typed(
            instance_actor::NAME,
            "1.0.2",
            instance_actor::instance_actor_1_0_2_orchestration,
        )
        // v1.0.3: Custom status + event queue for ad-hoc queries
        .register_versioned_typed(
            instance_actor::NAME,
            "1.0.3",
            instance_actor::instance_actor_1_0_3_orchestration,
        )
        .register_typed(
            system_pruner::NAME,
            system_pruner::system_pruner_1_0_0_orchestration,
        )
        // Register v1.0.1 of system pruner (2 min timer, keep 2 executions)
        .register_versioned_typed(
            system_pruner::NAME,
            "1.0.1",
            system_pruner::system_pruner_1_0_1_orchestration,
        )
        // Register v1.0.2 of system pruner (5 min timer, keep 2 executions)
        .register_versioned_typed(
            system_pruner::NAME,
            "1.0.2",
            system_pruner::system_pruner_1_0_2_orchestration,
        )
        // Register v1.0.3 of system pruner (uses system-prune-2, keep 3 executions)
        .register_versioned_typed(
            system_pruner::NAME,
            "1.0.3",
            system_pruner::system_pruner_1_0_3_orchestration,
        )
        // Register v1.0.4 of system pruner (1 min timer, uses system-prune-2, keep 3 executions)
        .register_versioned_typed(
            system_pruner::NAME,
            "1.0.4",
            system_pruner::system_pruner_1_0_4_orchestration,
        )
        // Register v1.0.5 of system pruner (custom status reporting)
        .register_versioned_typed(
            system_pruner::NAME,
            "1.0.5",
            system_pruner::system_pruner_1_0_5_orchestration,
        )
        // Image orchestrations
        .register_typed(
            create_image::NAME,
            create_image::create_image_orchestration,
        )
        // v1.0.1: Custom status reporting
        .register_versioned_typed(
            create_image_v1_0_1::NAME,
            "1.0.1",
            create_image_v1_0_1::create_image_1_0_1_orchestration,
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
            activities::deploy_postgres_v2::NAME,
            activities::deploy_postgres_v2::activity,
        )
        .register_typed(
            activities::deploy_postgres_from_pvc::NAME,
            activities::deploy_postgres_from_pvc::activity,
        )
        .register_typed(
            activities::create_pvc::NAME,
            activities::create_pvc::activity,
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
        .register_typed(
            activities::get_instance_password::NAME,
            activities::get_instance_password::activity,
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
        .register_typed(
            activities::cms::set_instance_runtime_image::NAME,
            activities::cms::set_instance_runtime_image::activity,
        )
        .register_typed(
            activities::cms::image_ops::NAME,
            activities::cms::image_ops::activity,
        )
        // Job activities (for backup/restore)
        .register_typed(
            activities::run_backup_job::NAME,
            activities::run_backup_job::activity,
        )
        .register_typed(
            activities::run_restore_job::NAME,
            activities::run_restore_job::activity,
        )
        .register_typed(
            activities::wait_for_job::NAME,
            activities::wait_for_job::activity,
        )
        .register_typed(
            activities::delete_job::NAME,
            activities::delete_job::activity,
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
        // Signaling activities
        .register_typed(
            activities::send_external_event::NAME,
            activities::send_external_event::activity,
        )
        // Query execution
        .register_typed(
            activities::execute_query::NAME,
            activities::execute_query::activity,
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
            registry.resolve_handler(system_pruner::NAME).is_some(),
            "SYSTEM_PRUNER should be registered in the orchestration registry"
        );
    }

    #[test]
    fn test_activity_registry_can_be_created() {
        let _registry = create_activity_registry();
        // Registry creation should not panic
    }
}
