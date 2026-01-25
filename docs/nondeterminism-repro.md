# Nondeterminism Bug Reproduction

## The Bug Pattern

Computing a value from two replayed `ctx.utcnow()` calls and using it as activity input.

## Minimal Reproduction

```rust
use duroxide::{OrchestrationContext, ActivityContext};
use std::time::Duration;
use serde::{Serialize, Deserialize};

// Activity input that includes a computed value
#[derive(Serialize, Deserialize)]
struct RecordMetricsInput {
    elapsed_ms: i32,  // ← This will differ on replay!
}

// The problematic orchestration
async fn buggy_orchestration(ctx: OrchestrationContext, _input: String) -> Result<(), String> {
    // Step 1: Record start time
    let start_time = ctx.utcnow().await?;  // Records SystemTime event #1
    
    // Step 2: Do some work (schedule an activity)
    ctx.schedule_activity("do_work", "{}").await?;  // Activity runs, completes
    
    // Step 3: Record end time
    let end_time = ctx.utcnow().await?;  // Records SystemTime event #2
    
    // Step 4: Compute elapsed time (THE BUG!)
    let elapsed_ms = end_time
        .duration_since(start_time)
        .unwrap()
        .as_millis() as i32;
    
    // Step 5: Use computed value as activity input
    let input = serde_json::to_string(&RecordMetricsInput { elapsed_ms }).unwrap();
    ctx.schedule_activity("record_metrics", &input).await?;  // ← FAILS ON REPLAY!
    
    Ok(())
}
```

## What Happens

### Original Execution

1. `ctx.utcnow()` → `T1 = 1000` (recorded in history)
2. `do_work` activity scheduled and completes
3. `ctx.utcnow()` → `T2 = 1007` (recorded in history)
4. Compute: `elapsed_ms = 1007 - 1000 = 7`
5. Schedule `record_metrics` with `{ elapsed_ms: 7 }` (recorded in history)

### Replay (after restart)

1. `ctx.utcnow()` → returns `T1 = 1000` from history ✓
2. `do_work` already in history → skip
3. `ctx.utcnow()` → returns `T2 = 1007` from history ✓
4. Compute: `elapsed_ms = 1007 - 1000 = 7` ... but wait!

The `SystemTime` values returned by duroxide have nanosecond precision internally.
On replay, the exact nanoseconds may differ due to:
- Floating point representation
- Serialization/deserialization precision
- Platform-specific SystemTime behavior

So you might get:
- Original: `T2 - T1 = 7.0001ms` → `as_millis()` → `7`
- Replay: `T2 - T1 = 6.9999ms` → `as_millis()` → `6`

**Result: Nondeterminism error!**

```
schedule mismatch: 
  action = { elapsed_ms: 6 } 
  vs 
  event = { elapsed_ms: 7 }
```

## The Fix

Move timing measurement **inside the activity** where it's not subject to replay:

```rust
// Activity measures its own duration
async fn do_work_with_timing(ctx: ActivityContext, input: String) -> Result<WorkOutput, String> {
    let start = std::time::Instant::now();  // Real wall clock, not replayed
    
    // ... do actual work ...
    
    let elapsed_ms = start.elapsed().as_millis() as i32;
    
    Ok(WorkOutput {
        result: "done".to_string(),
        elapsed_ms,  // ← Returned from activity, not computed in orchestration
    })
}

// Fixed orchestration
async fn fixed_orchestration(ctx: OrchestrationContext, _input: String) -> Result<(), String> {
    // Activity returns elapsed time
    let work_result: WorkOutput = ctx
        .schedule_activity_typed("do_work_with_timing", &input)
        .await?;
    
    // Use the activity's returned value - deterministic!
    let input = serde_json::to_string(&RecordMetricsInput { 
        elapsed_ms: work_result.elapsed_ms 
    }).unwrap();
    ctx.schedule_activity("record_metrics", &input).await?;
    
    Ok(())
}
```

## Key Principle

**Never compute values from replayed data and use them as activity/sub-orchestration inputs.**

Instead:
1. Have activities return any timing/computed data they measure
2. Or use `ctx.utcnow()` only for recording timestamps, not for computing deltas
3. Or accept that the value might differ and don't include it in activity inputs

## Real-World Example

In `instance_actor.rs`, the fix is:

```rust
// Before (buggy):
let start = ctx.utcnow().await?;
let result = ctx.schedule_activity("test_connection", ...).await?;
let end = ctx.utcnow().await?;
let response_time_ms = (end - start).as_millis();  // Computed!
ctx.schedule_activity("record", &RecordInput { response_time_ms, .. }).await?;

// After (fixed):
// Option 1: Have test_connection return response_time_ms
let result = ctx.schedule_activity("test_connection", ...).await?;
ctx.schedule_activity("record", &RecordInput { 
    response_time_ms: result.response_time_ms,  // From activity!
    .. 
}).await?;

// Option 2: Don't record response_time_ms at all from orchestration
// (measure it in the activity if needed)
```
