````skill
---
name: duroxide-orchestration-versioning
description: Guidance for safely versioning Duroxide orchestrations (immutability, adding new versions, registry registration, and rollout behavior).
---

# Duroxide Orchestration Versioning (Toygres)

## Non‑Negotiable Rule

**Orchestration code is immutable once deployed.**

Running instances replay historical events. If you change orchestration logic (or helpers it calls), replay can break in subtle ways.

This applies to:
- Orchestration functions (anything taking `OrchestrationContext`)
- Helper functions called by orchestrations (also orchestration code)
- Any change in activity scheduling order, parameters, retry policy, timers, branching logic, etc.

## When You MUST Create a New Version

Create a new orchestration version for **any** logic change, including "small fixes":
- Bug fixes, error handling tweaks, logging changes that affect control flow
- Changing activity calls (order/inputs/retry policies)
- Adding/removing timers
- Changing helper functions invoked by orchestrations

## Recommended Workflow (Diff‑Friendly)

When adding a new version, use this pattern to keep diffs readable:

Assume the current latest is `1.0.2` and the function is `myorch_1_0_2()`.

To bump to `1.0.3`:

1. **Copy** the current `myorch_1_0_2()` and paste it just below the existing function (this preserves the old implementation).
2. Go back to the original function (still in its original location), **rename it** from `myorch_1_0_2()` → `myorch_1_0_3()`.
3. Make your code changes inside the renamed `myorch_1_0_3()`.
4. Register the new version in the orchestration registry.

This keeps the "latest code" in the same spot in the file so git diffs show the real delta instead of a huge move/add.

Example:

```rust
// Step 1: Copy v1.0.2 just below (preserve old behavior)
pub async fn myorch_1_0_2(ctx: OrchestrationContext, input: Input) -> Result<Output, String> {
  ctx.trace_info("[v1.0.2] ...");
  // DO NOT MODIFY AFTER DEPLOY
}

// Step 2: Rename the original to v1.0.3 and change it in-place
pub async fn myorch_1_0_3(ctx: OrchestrationContext, input: Input) -> Result<Output, String> {
  ctx.trace_info("[v1.0.3] ...");
  // New logic
}
```

Registry:

```rust
OrchestrationRegistry::builder()
  .register_typed(NAME, my_orch_v1_0_0)                 // original
  .register_versioned_typed(NAME, "1.0.1", my_orch_1_0_1)
  .register_versioned_typed(NAME, "1.0.2", my_orch_1_0_2)
  .register_versioned_typed(NAME, "1.0.3", my_orch_1_0_3) // latest
  .build();
```

## Logging Convention

Prefix all orchestration logs with the version for debugging:
- `ctx.trace_info("[v1.0.3] ...")`

## Version Selection + Rollout Behavior

- `start_orchestration(...)` uses the **latest registered** version.
- Existing running instances stay on their current version until they naturally transition (often at `continue_as_new` boundaries, depending on the workflow design and Duroxide policy).

## Safe Refactors

If you want to "refactor" orchestration code:
- Do it by **adding a new version** (e.g., `1.0.5`) with the refactor.
- Do **not** modify earlier versions.

## Checklist

Before shipping:
- Added `*_1_0_{new}` function with `[vX.Y.Z]` logs
- Registered it via `.register_versioned_typed(NAME, "X.Y.Z", ...)`
- No changes to existing versioned orchestration functions
- `cargo build --workspace` passes

````
