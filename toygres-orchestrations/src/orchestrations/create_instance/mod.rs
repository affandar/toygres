/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::create-instance";

mod create_instance_orchestration;

pub use create_instance_orchestration::create_instance_1_0_5_orchestration;
