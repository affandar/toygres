/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::instance-actor";

pub mod instance_actor_1_0_0_orchestration;
mod instance_actor_orchestration;

pub use instance_actor_1_0_0_orchestration::instance_actor_1_0_0_orchestration;
pub use instance_actor_orchestration::instance_actor_1_0_1_orchestration;
