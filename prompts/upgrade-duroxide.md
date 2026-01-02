# Upgrade Duroxide Dependencies

**Purpose:** Analyze and upgrade `duroxide` and `duroxide-pg` dependencies to their latest versions.

---

## Step 1: Discover Current Versions

Read the workspace `Cargo.toml` to find the current versions:

```bash
grep -E "duroxide|duroxide-pg" Cargo.toml
```

Note the current versions of:
- `duroxide` (with any features enabled)
- `duroxide-pg`

---

## Step 2: Fetch Latest Versions and Changelogs

Fetch the latest release information from both repositories:

1. **Duroxide (core runtime):**
   - Latest version: https://crates.io/crates/duroxide
   - Changelog: https://github.com/affandar/duroxide/blob/main/CHANGELOG.md
   - README: https://github.com/affandar/duroxide/blob/main/README.md

2. **Duroxide-PG (PostgreSQL provider):**
   - Latest version: https://crates.io/crates/duroxide-pg
   - Changelog: https://github.com/affandar/duroxide-pg/blob/main/CHANGELOG.md
   - README: https://github.com/affandar/duroxide-pg/blob/main/README.md

Read these documents to determine:
- What is the latest version of each crate?
- What changes occurred between current and latest versions?

---

## Step 3: Analyze Breaking Changes

For each version between current and latest, categorize changes:

### Change Categories

1. **Runtime API Changes** - Changes to `OrchestrationContext`, `ActivityContext`, `Client`, etc.
   - These affect orchestration/activity code in `toygres-orchestrations`
   
2. **Provider Trait Changes** - Changes to the `Provider` trait interface
   - These are handled by `duroxide-pg`, not toygres directly
   - BUT: Require matching `duroxide-pg` version upgrade
   
3. **Schema/Migration Changes** - New database migrations in `duroxide-pg`
   - Check if new migrations are additive (safe) or destructive
   - `duroxide-pg` auto-applies migrations on startup
   
4. **Configuration Changes** - Changes to `RuntimeOptions` or other config
   - May require updates to server startup code

### Key Questions to Answer

- [ ] Are there breaking changes in the public API that toygres uses?
- [ ] Does `duroxide-pg` have new schema migrations?
- [ ] Are the schema changes additive (columns/procedures added) or destructive?
- [ ] Is a clean slate upgrade recommended or required?
- [ ] Are there new features toygres should leverage?

---

## Step 4: Present Upgrade Options

Based on the analysis, present the user with options:

### Option A: Runtime-Only Upgrade (When Safe)

**Use when:**
- Schema changes are additive (new columns/procedures)
- No destructive migrations
- duroxide-pg auto-migration will handle schema updates

**Procedure:**
```bash
# 1. Update Cargo.toml
# In workspace Cargo.toml, update versions:
# duroxide = { version = "=X.Y.Z", features = ["observability"] }
# duroxide-pg = { version = "=X.Y.Z" }

# 2. Rebuild
cargo build --workspace

# 3. Restart control plane (migrations auto-apply)
./scripts/stop-control-plane.sh
./scripts/start-control-plane.sh

# 4. Verify
./toygres server logs -f | head -50
# Look for migration success messages
```

### Option B: Clean Slate Upgrade (When Needed)

**Use when:**
- Destructive schema changes
- Major version upgrade with incompatible data formats
- Migration failures encountered
- User prefers fresh start

**Procedure:**
```bash
# 1. Stop and clean up all deployments
./scripts/cleanup-deployments.sh

# 2. Drop all schemas (CMS + Duroxide)
./scripts/drop-all-schemas.sh

# 3. Update Cargo.toml (as above)

# 4. Rebuild
cargo build --workspace

# 5. Reinitialize database
./scripts/db-init.sh
./scripts/db-migrate.sh

# 6. Start fresh
./scripts/start-control-plane.sh
```

**⚠️ Warning:** This destroys all existing orchestration state and instance metadata.

---

## Step 5: Identify New Features to Adopt

Beyond breaking changes, scan the changelogs for **new features** that could improve toygres:

### Feature Categories to Look For

1. **Activity Improvements**
   - Cancellation support (`ctx.is_cancelled()`, `ctx.cancelled()`)
   - Retry policies (`schedule_activity_with_retry`)
   - Timeout configurations
   - Lock renewal improvements

2. **Orchestration Improvements**
   - New composition patterns (`select`, `join` improvements)
   - Continue-as-new enhancements
   - Sub-orchestration improvements
   - External event handling

3. **Observability Improvements**
   - New metrics
   - Tracing enhancements
   - Log format changes
   - Dashboard improvements

4. **Client API Improvements**
   - New status types
   - Better error details
   - Management API additions

### Propose Toygres Enhancements

For each new feature, consider:

#### A. Orchestration/Activity Changes

```
Feature: [Name from changelog]
Current toygres behavior: [How it works now]
Proposed change: [How to leverage the new feature]
Files to modify:
  - toygres-orchestrations/src/activities/[file].rs
  - toygres-orchestrations/src/orchestrations/[file].rs
Benefit: [Why this improves toygres]
```

**Examples of adoptable patterns:**

1. **Activity Cancellation** - Add `ctx.is_cancelled()` checks to:
   - Long-running K8s watch operations
   - Health monitoring loops in `instance_actor`
   - Any activity with internal polling

2. **Retry Policies** - Replace manual retry loops with `schedule_activity_with_retry`:
   - K8s API calls that may fail transiently
   - Database operations
   - External service calls

3. **Typed APIs** - Convert string-based to typed calls:
   - Use `schedule_activity_typed` instead of JSON serialization
   - Use `schedule_sub_orchestration_typed` for child workflows

#### B. UI/UX Enhancements

Consider if new runtime features enable UI improvements:

```
Feature: [Name]
UI Enhancement: [What to add/change in toygres-ui]
Files to modify:
  - toygres-ui/src/components/[Component].tsx
  - toygres-ui/src/api/[api].ts
Mockup: [Brief description of UI change]
```

**Examples:**

1. **Better Error Classification** - If duroxide adds `ErrorDetails.category()`:
   - Show error category badges in instance list
   - Color-code errors (infrastructure=red, application=yellow, config=orange)
   - Add filtering by error type

2. **Orchestration Metrics** - If new metrics are exposed:
   - Add dashboard widgets for success rates
   - Show activity duration histograms
   - Display retry counts

3. **Cancellation Status** - If cancellation state is queryable:
   - Add "Cancelling..." status to instance cards
   - Show cancel button with confirmation
   - Display grace period countdown

#### C. API Enhancements

Consider if new client APIs should be exposed:

```
Feature: [Name]
API Enhancement: [New endpoint or response change]
Files to modify:
  - toygres-server/src/api.rs
  - toygres-ui/src/api/[file].ts
```

**Examples:**

1. **System Metrics** - Expose `client.get_system_metrics()`:
   - `GET /api/server/metrics` endpoint
   - Dashboard showing total/running/failed counts

2. **Queue Depths** - Expose `client.get_queue_depths()`:
   - `GET /api/server/queues` endpoint
   - Show orchestrator/worker queue health

3. **Execution History** - Expose `client.read_execution_history()`:
   - `GET /api/instances/:name/history` endpoint
   - Timeline view of orchestration events

---

## Step 6: Check for Code Updates Needed

After identifying breaking changes, search the codebase for affected patterns:

```bash
# Example: If ActivityContext API changed
grep -r "ActivityContext" toygres-orchestrations/src/

# Example: If schedule_activity signature changed  
grep -r "schedule_activity" toygres-orchestrations/src/

# Example: If RuntimeOptions changed
grep -r "RuntimeOptions" toygres-server/src/
```

Document any code changes required before the upgrade can proceed.

---

## Step 7: Verification Checklist

After upgrading, verify:

```bash
# 1. Build succeeds
cargo build --workspace

# 2. Tests pass
cargo test --workspace

# 3. Server starts
./scripts/start-control-plane.sh

# 4. Health check passes
curl http://localhost:8080/health

# 5. Can create/delete instances
curl -X POST http://localhost:8080/api/instances \
  -H "Content-Type: application/json" \
  -d '{"name": "test-upgrade", "admin_username": "admin", "admin_password": "test123"}'

curl http://localhost:8080/api/server/orchestrations

curl -X DELETE http://localhost:8080/api/instances/test-upgrade
```

---

## Reference: Key Documentation

When analyzing upgrades, consult these documents:

| Document | URL | Purpose |
|----------|-----|---------|
| Duroxide CHANGELOG | https://github.com/affandar/duroxide/blob/main/CHANGELOG.md | Breaking changes, new features |
| Duroxide-PG CHANGELOG | https://github.com/affandar/duroxide-pg/blob/main/CHANGELOG.md | Schema migrations, API changes |
| Orchestration Guide | https://github.com/affandar/duroxide/blob/main/docs/ORCHESTRATION-GUIDE.md | API reference, patterns |
| Cross-Crate Registry | https://github.com/affandar/duroxide/blob/main/docs/cross-crate-registry-pattern.md | Registry patterns |
| Provider Implementation | https://github.com/affandar/duroxide/blob/main/docs/provider-implementation-guide.md | Provider trait details |

---

## Reference: Common Breaking Change Patterns

### Provider Trait Changes (Handled by duroxide-pg)

These don't require toygres code changes, but require matching duroxide-pg upgrade:

- `fetch_work_item` return type changes
- `fetch_orchestration_item` return type changes  
- `ack_*` method signature changes
- New required trait methods

### Runtime API Changes (May Require Code Changes)

- `OrchestrationContext` method additions/changes
- `ActivityContext` method additions/changes
- `RuntimeOptions` field additions/changes
- `Client` API changes

### New Features to Consider Adopting

When upgrading, check if new features would benefit toygres:

- **Activity cancellation** - Add `ctx.is_cancelled()` checks to long-running activities
- **Retry policies** - Use `schedule_activity_with_retry` for unreliable operations
- **Typed APIs** - Convert string-based to typed orchestration/activity calls
- **Observability** - Leverage new metrics or tracing capabilities
- **Error classification** - Use `ErrorDetails.category()` for better error handling

---

## Output Format

Present your analysis as:

```
## Upgrade Analysis: duroxide/duroxide-pg

### Current Versions
- duroxide: X.Y.Z
- duroxide-pg: X.Y.Z

### Latest Versions  
- duroxide: X.Y.Z (released YYYY-MM-DD)
- duroxide-pg: X.Y.Z (released YYYY-MM-DD)

### Upgrade Needed: YES/NO

### Breaking Changes
1. [BREAKING/FEATURE/FIX] Description...
2. ...

### Schema Migrations
- Migration XXXX: Description (additive/destructive)
- ...

### Code Changes Required
- [ ] File: path/to/file.rs - Change needed
- [ ] ...

### Recommended Upgrade Path
Option A/B with rationale...

### Rollback Safety
Safe/Unsafe - explanation...

### New Features to Adopt

#### Immediate (implement during upgrade):
1. **Feature Name**
   - What: Brief description
   - Where: Files to modify
   - Benefit: Why it helps

#### Future (implement after upgrade):
1. **Feature Name**
   - What: Brief description
   - Where: Files to modify
   - Benefit: Why it helps

### UI/UX Enhancements Enabled

1. **Enhancement Name**
   - Component: path/to/component.tsx
   - Change: What to add/modify
   - Mockup: Brief description

### API Enhancements Enabled

1. **Endpoint Name**
   - Route: GET/POST /api/...
   - Purpose: What it exposes
   - Implementation: Brief notes
```
