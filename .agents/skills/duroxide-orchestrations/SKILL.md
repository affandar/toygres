---
name: duroxide-orchestrations
description: Writing durable workflows using Duroxide in Rust. Use when creating orchestrations, activities, workflows, or when the user mentions duroxide, durable functions, or workflow orchestration.
---

# Duroxide Durable Workflow Development

## Overview

Skills for developing durable workflows using Duroxide in Rust. Duroxide provides deterministic, replayable orchestrations with automatic failure recovery.

## Core Concepts

- **Activities**: Idempotent operations that perform actual work (K8s calls, DB queries, HTTP requests)
- **Orchestrations**: Deterministic workflow logic that coordinates activities
- **Continue-as-new**: Pattern for long-running orchestrations to prevent unbounded history
- **Sub-orchestrations**: Reusable workflow compositions
- **Detached orchestrations**: Background workflows that run independently

## Directory Structure

```
toygres-orchestrations/src/
├── orchestrations/          # Workflow definitions
│   └── my_orchestration.rs
├── activities/              # Atomic operations
│   ├── my_activity.rs
│   └── cms/                 # Grouped by domain
│       └── my_cms_activity.rs
├── registry.rs              # Central registration
├── types.rs                 # Orchestration I/O types
├── activity_types.rs        # Activity I/O types
└── names.rs                 # Naming constants
```

## Naming Convention

Follow the hierarchical namespace pattern in `names.rs`:

```rust
// Format: {crate}::{type}::{name}
pub mod orchestrations {
    pub const MY_WORKFLOW: &str = "toygres-orchestrations::orchestration::my-workflow";
}

pub mod activities {
    pub const MY_ACTIVITY: &str = "toygres-orchestrations::activity::my-activity";
}
```

## Creating Activities

Activities must be **idempotent** - safe to retry without side effects.

```rust
// toygres-orchestrations/src/activities/my_activity.rs
use duroxide::ActivityContext;
use crate::activity_types::{MyInput, MyOutput};

/// Activity name for registration and scheduling
pub const NAME: &str = "toygres-orchestrations::activity::my-activity";

pub async fn activity(
    ctx: ActivityContext,
    input: MyInput,
) -> Result<MyOutput, String> {
    ctx.trace_info(format!("Starting activity: {}", input.name));

    // CRITICAL: Check idempotency first - has this already been done?
    let already_done = check_if_done(&input).await?;
    if already_done {
        ctx.trace_info("Already completed, returning cached result");
        return Ok(MyOutput { ... });
    }

    // Perform actual work
    let result = do_work(&input).await
        .map_err(|e| format!("Failed: {}", e))?;

    ctx.trace_info("Activity completed successfully");
    Ok(result)
}
```

### Activity Registration

Add to `registry.rs`:

```rust
pub fn create_activity_registry() -> ActivityRegistry {
    ActivityRegistry::builder()
        .register_typed(
            activities::my_activity::NAME,
            activities::my_activity::activity,
        )
        .build()
}
```

## Creating Orchestrations

Orchestrations coordinate activities with deterministic logic.

```rust
// toygres-orchestrations/src/orchestrations/my_orchestration.rs
use duroxide::{OrchestrationContext, RetryPolicy, BackoffStrategy};
use std::time::Duration;

pub async fn my_orchestration(
    ctx: OrchestrationContext,
    input: MyInput,
) -> Result<MyOutput, String> {
    ctx.trace_info(format!("Starting orchestration: {}", input.id));

    // Schedule an activity (basic)
    let result = ctx
        .schedule_activity_typed::<ActivityInput, ActivityOutput>(
            activities::my_activity::NAME,
            &activity_input,
        )
        .await?;

    Ok(MyOutput { ... })
}
```

### Orchestration Registration

```rust
pub fn create_orchestration_registry() -> OrchestrationRegistry {
    OrchestrationRegistry::builder()
        .register_typed(
            orchestrations::MY_WORKFLOW,
            crate::orchestrations::my_orchestration::my_orchestration,
        )
        .build()
}
```

## Scheduling Patterns

### Basic Activity Scheduling

```rust
let result = ctx
    .schedule_activity_typed::<Input, Output>(NAME, &input)
    .await?;
```

### Activity with Retry and Backoff

```rust
let result = ctx
    .schedule_activity_with_retry_typed::<Input, Output>(
        NAME,
        &input,
        RetryPolicy::new(5)  // Max 5 retries
            .with_backoff(BackoffStrategy::Exponential {
                base: Duration::from_secs(2),
                multiplier: 2.0,
                max: Duration::from_secs(30),
            })
            .with_timeout(Duration::from_secs(60)),
    )
    .await?;
```

### Sub-Orchestration (Reusable Workflow)

```rust
let result = ctx
    .schedule_sub_orchestration_typed::<Input, Output>(
        orchestrations::CHILD_WORKFLOW,
        &input,
    )
    .await?;
```

### Detached Orchestration (Background/Fire-and-Forget)

```rust
// Start orchestration without waiting for completion
let input_json = serde_json::to_string(&input)?;
ctx.schedule_orchestration(
    orchestrations::BACKGROUND_WORKFLOW,
    &orchestration_id,  // Unique ID for this instance
    input_json,
);
// Continues immediately - orchestration runs independently
```

## Deterministic Timing

**NEVER use `tokio::time::sleep()` - it breaks determinism!**

```rust
// Deterministic timer - safe for replay
ctx.schedule_timer(Duration::from_secs(30)).await;

// Get current time deterministically
let now = ctx.utc_now().await?;
```

## Signal Handling with select2

Wait for either a timer or an external signal:

```rust
// Wait for 30 seconds OR deletion signal (whichever comes first)
let timer = ctx.schedule_timer(Duration::from_secs(30));
let deletion_signal = ctx.schedule_wait("InstanceDeleted");

let (winner_index, _) = ctx.select2(timer, deletion_signal).await;

if winner_index == 1 {
    // Signal received
    ctx.trace_info("Received signal, exiting gracefully");
    return Ok(());
}
// Timer fired, continue
```

## Continue-as-New Pattern

For long-running orchestrations, prevent unbounded history growth:

```rust
pub async fn long_running_orchestration(
    ctx: OrchestrationContext,
    input: MyInput,
) -> Result<MyOutput, String> {
    // Do one iteration of work
    let result = do_work(&ctx, &input).await?;

    // Wait before next iteration
    ctx.schedule_timer(Duration::from_secs(60)).await;

    // Continue as new: restarts with fresh execution history
    let next_input = MyInput {
        iteration: input.iteration + 1,
        ..input
    };
    let input_json = serde_json::to_string(&next_input)?;
    ctx.continue_as_new(input_json).await?;

    Ok(result)
}
```

## Error Handling Patterns

### Propagate Critical Errors

```rust
// Fail orchestration if activity fails
let result = ctx.schedule_activity_typed::<I, O>(NAME, &input).await?;
```

### Best-Effort Operations (Log and Continue)

```rust
// Don't fail orchestration for non-critical operations
if let Err(err) = ctx.schedule_activity_typed::<I, O>(NAME, &input).await {
    ctx.trace_warn(format!("Non-critical operation failed: {}", err));
    // Continue despite error
}
```

## Versioning Strategy

Versioning is critical and detailed enough to warrant its own skill.

See: `.agents/skills/duroxide-orchestration-versioning/SKILL.md`

## Logging

Use context logging methods for durability:

```rust
ctx.trace_info(format!("Processing: {}", id));
ctx.trace_warn(format!("Warning: {}", message));
ctx.trace_error(format!("Error: {}", error));
```

## Best Practices Summary

1. **Naming**: Use `{crate}::{type}::{name}` format in `names.rs`
2. **Idempotency**: Activities must be safe to retry
3. **Determinism**: Only use `ctx.schedule_timer()`, never `tokio::time::sleep()`
4. **Versioning**: Never modify existing orchestrations - create new versions
5. **Error Handling**: Propagate critical errors, log non-critical ones
6. **Long-Running**: Use continue-as-new to prevent history bloat
7. **Testing**: Add serialization round-trip tests for all types
8. **Logging**: Use `ctx.trace_*()` methods, not `println!`
9. **Composition**: Use sub-orchestrations for reusable workflows
10. **Background Tasks**: Use detached orchestrations for fire-and-forget

