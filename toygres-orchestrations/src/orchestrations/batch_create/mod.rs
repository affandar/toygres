/// Batch Create Orchestration
///
/// Starts N create-instance orchestrations as detached children,
/// then monitors their progress. Keeps custom_status updated with
/// aggregated progress including error messages from failures.

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::batch-create";

mod batch_create_orchestration;

pub use batch_create_orchestration::batch_create_1_0_0_orchestration;
