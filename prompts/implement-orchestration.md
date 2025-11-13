# Prompt: Implement Duroxide Orchestration

## Context

I'm working on the Toygres project, a Rust-based control plane for hosting PostgreSQL containers on AKS using the Duroxide durable workflow framework.

**Project Structure**: Cargo workspace with `toygres-models`, `toygres-activities`, `toygres-orchestrations`, and `toygres-server` crates.

**What are Orchestrations?** Orchestrations in Duroxide are durable workflows that coordinate activities. They should:
- Be deterministic (same inputs always produce same sequence of calls)
- Use Duroxide's orchestration context for all external calls
- Call activities for any non-deterministic operations
- Use timers for delays (not tokio::sleep)
- Be resumable after interruptions
- Follow the orchestration pattern from Duroxide docs

## Task

Implement the `[ORCHESTRATION_NAME]` in `toygres-orchestrations/src/[file_name].rs`.

**Purpose**: [Describe what this orchestration does]

**Inputs**: [List input parameters and types]

**Outputs**: [Describe return type]

**Workflow Steps**: [List the sequence of activities to call]

## Requirements

1. Define the orchestration input/output structs with `Serialize`/`Deserialize`
2. Implement the orchestration following Duroxide patterns
3. Call activities using the orchestration context
4. Use Duroxide timers for delays (e.g., polling, retries)
5. Handle errors appropriately (some may need retries)
6. Add logging for workflow progress
7. Make it deterministic - no direct I/O, random numbers, or system time
8. Export the orchestration from `toygres-orchestrations/src/lib.rs`

## Key Duroxide Concepts

**Determinism**: Orchestrations must be deterministic. This means:
- ❌ Don't use `tokio::sleep` (use Duroxide timers)
- ❌ Don't call external APIs directly (use activities)
- ❌ Don't use random numbers or system time directly
- ✅ All external operations through activities
- ✅ Use orchestration context for scheduling

**Detached Orchestrations**: To start another orchestration that runs independently:
```rust
// Start a detached orchestration
let child_id = ctx.start_orchestration::<ChildOrchestration>(input).await?;
// Store child_id if you need to cancel it later
```

**Cancellation**: To cancel a running orchestration:
```rust
ctx.cancel_orchestration(orchestration_id).await?;
```

## Example Pattern

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;
use duroxide::{OrchestrationContext, Orchestration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyOrchestrationInput {
    pub param1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyOrchestrationOutput {
    pub result: String,
}

pub struct MyOrchestration;

impl Orchestration for MyOrchestration {
    type Input = MyOrchestrationInput;
    type Output = MyOrchestrationOutput;
    
    async fn run(ctx: OrchestrationContext, input: Self::Input) -> Result<Self::Output> {
        info!("Starting MyOrchestration");
        
        // Call activities through context
        let activity_result = ctx.call_activity::<SomeActivity>(activity_input).await?;
        
        // Use Duroxide timer for delays
        ctx.create_timer(std::time::Duration::from_secs(30)).await?;
        
        // Call more activities as needed
        
        Ok(MyOrchestrationOutput {
            result: "success".to_string(),
        })
    }
}
```

## Additional Context

[Add any specific details about the orchestration you're implementing, such as:
- Which activities to call
- Polling logic requirements
- Error handling strategies
- Whether it needs to start detached child orchestrations
]

## Important for Toygres Orchestrations

**CreateInstanceOrchestration**: Must start a detached HealthCheckOrchestration and store its ID in metadata.

**DeleteInstanceOrchestration**: Must retrieve and cancel the health check orchestration before cleanup.

**HealthCheckOrchestration**: Runs in an infinite loop with 30-second delays. Only terminates when cancelled.

