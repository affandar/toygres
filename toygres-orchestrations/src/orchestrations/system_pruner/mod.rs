/// System Pruner Orchestration
///
/// A continuously-running system orchestration that performs maintenance:
/// - Deletes terminal (Completed/Failed) orchestration instances older than 6 hours
/// - Prunes all executions to keep only the current one (for long-running actors)
///
/// This orchestration uses the continue-as-new pattern and runs every 1 hour.
/// It logs all prune/delete operations for debugging.

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::system-pruner";

pub mod system_pruner_1_0_0_orchestration;
pub mod system_pruner_1_0_1_orchestration;
pub mod system_pruner_1_0_2_orchestration;
pub mod system_pruner_1_0_3_orchestration;
mod system_pruner_orchestration;

pub use system_pruner_1_0_0_orchestration::system_pruner_1_0_0_orchestration;
pub use system_pruner_1_0_1_orchestration::system_pruner_1_0_1_orchestration;
pub use system_pruner_1_0_2_orchestration::system_pruner_1_0_2_orchestration;
pub use system_pruner_1_0_3_orchestration::system_pruner_1_0_3_orchestration;
pub use system_pruner_orchestration::system_pruner_1_0_4_orchestration;
