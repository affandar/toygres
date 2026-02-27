/// Create Semaphore Orchestration
///
/// An eternal singleton that acts as a global concurrency gate for create-instance
/// orchestrations. Limits concurrent creates to `max_concurrent` (default 10).
///
/// Communication:
/// - Receives Acquire/Release messages via "permits" event queue
/// - Grants permits by sending "create-permit" named event back to requester
///   via the send-external-event activity
/// - Uses continue-as-new every 50 events to bound history growth

/// Orchestration name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::orchestration::create-semaphore";

mod create_semaphore_orchestration;

pub use create_semaphore_orchestration::create_semaphore_1_0_0_orchestration;
