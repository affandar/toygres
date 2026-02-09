//! Delete PostgreSQL instance orchestration

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::delete-instance";

pub mod delete_instance_1_0_0_orchestration;
pub mod delete_instance_1_0_1_orchestration;
mod delete_instance_orchestration;

pub use delete_instance_1_0_0_orchestration::delete_instance_1_0_0_orchestration;
pub use delete_instance_1_0_1_orchestration::delete_instance_1_0_1_orchestration;
pub use delete_instance_orchestration::delete_instance_1_0_2_orchestration;

use duroxide::OrchestrationContext;
use crate::activities::cms;
use crate::activity_types::{
    UpdateInstanceStateInput, UpdateInstanceStateOutput,
    FreeDnsNameInput, FreeDnsNameOutput,
    DeleteInstanceRecordInput, DeleteInstanceRecordOutput,
};

pub(crate) async fn update_cms_state(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) {
    if let Err(err) = ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .await
    {
        ctx.trace_warn(format!("Failed to update CMS state: {}", err));
    }
}

pub(crate) async fn update_cms_state_strict(
    ctx: &OrchestrationContext,
    update_input: UpdateInstanceStateInput,
) -> Result<(), String> {
    ctx
        .schedule_activity_typed::<UpdateInstanceStateInput, UpdateInstanceStateOutput>(
            cms::update_instance_state::NAME,
            &update_input,
        )
        .await
        .map_err(|e| format!("Failed to update CMS state: {}", e))?;
    Ok(())
}

pub(crate) async fn free_dns_name(
    ctx: &OrchestrationContext,
    k8s_name: &str,
) {
    if let Err(err) = ctx
        .schedule_activity_typed::<FreeDnsNameInput, FreeDnsNameOutput>(
            cms::free_dns_name::NAME,
            &FreeDnsNameInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .await
    {
        ctx.trace_warn(format!("Failed to free DNS name: {}", err));
    }
}

pub(crate) async fn delete_cms_record(
    ctx: &OrchestrationContext,
    k8s_name: &str,
) {
    ctx.trace_info("Deleting CMS record (triggers instance actor completion)");

    if let Err(err) = ctx
        .schedule_activity_typed::<DeleteInstanceRecordInput, DeleteInstanceRecordOutput>(
            cms::delete_instance_record::NAME,
            &DeleteInstanceRecordInput {
                k8s_name: k8s_name.to_string(),
            },
        )
        .await
    {
        ctx.trace_warn(format!("Failed to delete CMS record: {}", err));
    } else {
        ctx.trace_info("CMS record deleted, instance actor will complete on next iteration");
    }
}
