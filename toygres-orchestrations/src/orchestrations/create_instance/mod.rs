/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::create-instance";

mod create_instance_1_0_5_orchestration;
mod create_instance_orchestration;
mod create_instance_1_0_7_orchestration;

pub use create_instance_1_0_5_orchestration::create_instance_1_0_5_orchestration;
pub use create_instance_orchestration::create_instance_1_0_6_orchestration;
pub use create_instance_1_0_7_orchestration::create_instance_1_0_7_orchestration;
