# Toygres - AI Coding Assistant Instructions

> **🚨 STOP: DO NOT `git commit` OR `git push` WITHOUT EXPLICIT USER PERMISSION 🚨**
>
> Always ask before committing or pushing. Never assume.

## Project Overview

Toygres is a **Rust-based control plane** for hosting PostgreSQL containers as a service on Azure Kubernetes Service (AKS). It uses [Duroxide](https://github.com/affandar/duroxide) for durable workflow orchestration.

## ⚠️ Implementation Guidelines

**Do not commit or push** unless explicitly asked by the user.

**No half-baked features.** When implementing a new capability:
1. **Don't add activities** unless they are called by an orchestration
2. **Don't add orchestrations** unless they are invoked by the server API
3. **Don't add API endpoints** unless they are exposed in the UI
4. Every feature must be **end-to-end complete**: UI → API → Orchestration → Activities (if durable) or UI → API → K8s (if simple/atomic)

**Before removing "unused" activities:**
1. Search for the activity's `NAME` constant in orchestrations, not just the module path:
   ```bash
   grep -r "activity_name" toygres-orchestrations/src/orchestrations/
   ```
2. Activities may be referenced via `cms::activity_name::NAME` pattern (not `activities::`)
3. Check both direct calls and `schedule_activity_typed` / `schedule_activity_with_retry_typed` invocations
4. **Always run `cargo build` after removals** before committing to catch missed references

## Architecture

### Crate Structure
- **`toygres-models`** - Shared data types (`InstanceState`, `HealthStatus`, `DeploymentConfig`)
- **`toygres-orchestrations`** - Duroxide activities and orchestrations (the core business logic)
- **`toygres-server`** - Axum REST API + Duroxide worker runtime
- **`toygres-ui`** - React/TypeScript dashboard (Vite + TailwindCSS)

### Key Patterns

**Duroxide Activities** (atomic operations in `toygres-orchestrations/src/activities/`):
- Activities are registered with a `NAME` constant following pattern: `"toygres-orchestrations::activity::{name}"`
- Each activity file exports: `pub const NAME`, `pub async fn activity(ctx, input) -> Result<Output, String>`
- Activities wrap K8s operations (`deploy_postgres`, `delete_postgres`) or CMS database updates (`cms/` subdirectory)

**Duroxide Orchestrations** (durable workflows in `toygres-orchestrations/src/orchestrations/`):
- `create_instance` - Deploys PostgreSQL StatefulSet, waits for ready, tests connection
- `delete_instance` - Cancels instance actor, removes K8s resources
- `instance_actor` - Detached orchestration for continuous health monitoring (runs forever via continue-as-new)

**Registry Pattern** - All activities/orchestrations are registered in `toygres-orchestrations/src/registry.rs`:
```rust
ActivityRegistry::builder()
    .register_typed(activities::deploy_postgres::NAME, activities::deploy_postgres::activity)
```

**K8s Templates** - YAML templates in `toygres-orchestrations/src/templates/` use Tera templating:
- `postgres-statefulset.yaml`, `postgres-service.yaml`, `postgres-pvc.yaml`

### Database

**CMS Schema** (`toygres_cms`): Stores instance metadata - see `migrations/cms/0001_initial_schema.sql`
**Duroxide Schema** (`toygres_duroxide`): Managed by duroxide-pg for workflow state

**Idempotency Requirement**: All CMS activities must be idempotent for Duroxide replay safety. Use:
- `ON CONFLICT DO UPDATE` for creates
- Conditional updates checking current state before updating

## Development Workflow

### Local Development
```bash
# Run unit tests
cargo test --workspace

# Run the full control plane locally (observability + backend + UI)
./scripts/start-control-plane.sh    # Starts on :8080 (API) and :3000 (UI)
./scripts/stop-control-plane.sh     # Stops everything
```

**Testing Complex Changes Locally**: For complex changes (new orchestrations, activities, or multi-step workflows), test locally first using `./scripts/start-control-plane.sh` before deploying to AKS. This provides:
- Faster iteration (no image build/push cycle)
- Debug-level logging by default (vs info in AKS)
- Easier log access via `./toygres server logs -f`

To pause AKS while testing locally: `kubectl scale deployment toygres-server -n toygres-system --replicas=0`

### AKS Deployment
```bash
# Deploy to AKS (builds images, pushes to ACR, applies K8s manifests)
./deploy/deploy-to-aks.sh           # HTTP only
./deploy/deploy-to-aks.sh --https   # With Let's Encrypt SSL

# Restart server in AKS (if deployment doesn't pick up changes)
kubectl rollout restart deployment/toygres-server -n toygres-system
kubectl rollout status deployment/toygres-server -n toygres-system

# View AKS logs
kubectl logs -n toygres-system -l app.kubernetes.io/component=server -f
```

### Database Setup (required before first run)
```bash
./scripts/db-init.sh                # Creates CMS schema
./scripts/db-migrate.sh             # Runs incremental migrations
```

### Other Commands
```bash
cargo build --workspace             # Build all crates
./toygres server start              # Start daemon locally (API on :8080)
./toygres server logs -f            # Tail server logs
./toygres server stop               # Stop local daemon
cd toygres-ui && npm run dev        # Frontend dev server on :3000
```

## API Endpoints

REST API on `:8080` - see `toygres-server/src/api.rs`:
- `POST /api/instances` - Start `CreateInstanceOrchestration`
- `DELETE /api/instances/:name` - Start `DeleteInstanceOrchestration`
- `POST /api/instances/:name/stop` - Stop instance (scale to 0 replicas)
- `POST /api/instances/:name/start` - Start instance (scale to 1 replica)
- `POST /api/instances/:name/restart` - Restart instance (rollout restart)
- `GET /api/instances` - List instances from CMS
- `GET /api/server/orchestrations` - List all Duroxide orchestrations
- `GET /api/instances` - List instances from CMS
- `GET /api/server/orchestrations` - List all Duroxide orchestrations

## Code Conventions

- **Error handling**: Use `anyhow::Result` in server code, `Result<T, String>` in activities/orchestrations
- **Async**: All I/O is async with Tokio
- **Tracing**: Use `tracing` macros, server logs to `~/.toygres/server.log`
- **Activity naming**: `"crate-name::activity::kebab-case-name"`
- **Orchestration naming**: `"crate-name::orchestration::kebab-case-name"`
- **Input/Output types**: Defined in `toygres-orchestrations/src/types.rs` and `activity_types.rs`

## Debugging Tips

**Extract detailed PostgreSQL errors**: When using `tokio-postgres`, extract the full `db_error` for diagnostics:
```rust
if let Some(db_err) = e.as_db_error() {
    format!("severity={}, code={}, message={}", 
        db_err.severity(), db_err.code().code(), db_err.message())
}
```
Common codes: `28P01` = password authentication failed, `28000` = invalid authorization

**Test assumptions with direct connections**: Before adding complex workarounds, verify assumptions by connecting directly:
```bash
PGPASSWORD=<password> psql "postgresql://postgres@<ip>:5432/postgres?gssencmode=disable" -c "SELECT 1"
```

## Adding New Features

**New Activity**:
1. Create file in `toygres-orchestrations/src/activities/`
2. Define `pub const NAME`, input/output types, `pub async fn activity(...)`
3. Register in `registry.rs` via `.register_typed()`
4. Add to `activities/mod.rs`

**New Orchestration**:
1. Create file in `toygres-orchestrations/src/orchestrations/`
2. Define orchestration function and add to `names.rs`
3. Register in `registry.rs`
4. Add to `orchestrations/mod.rs`

## Duroxide Orchestration Versioning

**⚠️ CRITICAL: Orchestration code is IMMUTABLE once deployed**

Orchestrations are durable workflows that may be running for hours, days, or indefinitely. Changing orchestration code without proper versioning will break running instances and cause undefined behavior.

**How to Identify Orchestration Code:**
1. **Function signature**: Takes `OrchestrationContext` (or `ctx: OrchestrationContext`) as first parameter
2. **Function name**: Contains `_orchestration` suffix (e.g., `create_instance_orchestration`)
3. **File location**: Lives in `toygres-orchestrations/src/orchestrations/`
4. **Helper functions**: Any `async fn` in orchestration files that takes `&OrchestrationContext` is also orchestration code

**⚠️ NEVER modify existing orchestration functions directly. Always create a new version.**

**When You MUST Create a New Version:**
- ANY change to orchestration logic, even "minor" fixes
- Changes to helper functions called by orchestrations
- Changes to activity call order or parameters within orchestrations
- Bug fixes, error handling changes, logging changes - ALL require versioning

**Registering Versioned Orchestrations**:
```rust
// Default version (1.0.0) - use register_typed
.register_typed(orchestrations::MY_ORCH, my_orchestration)

// Explicit versions - use register_versioned_typed
.register_versioned_typed(orchestrations::MY_ORCH, "1.0.1", my_orchestration_1_0_1)
.register_versioned_typed(orchestrations::MY_ORCH, "1.0.2", my_orchestration_1_0_2)
```

**Version Upgrade Behavior**:
- Default policy is **"Latest"** - `continue_as_new` automatically upgrades to newest registered version
- Version upgrade happens **at `continue_as_new` time**, not when server restarts
- If old version has a timer running, it must complete before upgrade occurs
- Example: v1.0.1 with 2-min timer must wait for timer to expire before upgrading to v1.0.2

**Version Upgrade Timing** (why UI shows old version for a while):
```
T+0:00  Server restarts with v1.0.2 registered
        But v1.0.1 was mid-cycle with ~1 min left on its timer
T+1:00  v1.0.1's timer expires, does its work
T+1:01  v1.0.1 calls continue_as_new() → Duroxide resolves "Latest" → v1.0.2
        Database updated: orchestration_version = "1.0.2"
T+1:02  UI refreshes, now shows v1.0.2
```

**Required Steps for Versioned Orchestrations**:
1. Keep the same orchestration NAME constant across versions
2. Create separate functions per version: `my_orch()`, `my_orch_1_0_1()`, `my_orch_1_0_2()`
3. Add version prefix to trace logs for debugging: `ctx.trace_info("[v1.0.2] Starting...")`
4. Register all versions in `registry.rs` - old versions for running instances, new for future
5. Version info flows: Duroxide `InstanceInfo.orchestration_version` → Backend API → UI

## Environment Variables

Required in `.env`:
- `DATABASE_URL` - PostgreSQL connection for CMS + Duroxide state
- `AKS_CLUSTER_NAME`, `AKS_RESOURCE_GROUP` - Azure/K8s configuration

## Duroxide RuntimeOptions Tuning

Key settings in `toygres-server/src/main.rs` `RuntimeOptions`:
- `dispatcher_min_poll_interval`: 1 second (default 100ms is too aggressive for production)
- `orchestrator_lock_timeout`: 30 seconds (default 5s too short, causes poison messages on slow DB)
- `worker_lock_timeout`: 300 seconds (for long-running activities)
- `orchestration_concurrency`: 10
- `worker_concurrency`: 10

**Poison Message Prevention**: If lock timeout is too short and DB is slow, orchestrations can't complete their turn before the lock expires, causing infinite retry loops. Increase `orchestrator_lock_timeout` if you see repeated failures.

## Key Files

- [toygres-orchestrations/src/registry.rs](toygres-orchestrations/src/registry.rs) - Activity/orchestration registration
- [toygres-orchestrations/src/names.rs](toygres-orchestrations/src/names.rs) - Orchestration name constants
- [toygres-server/src/api.rs](toygres-server/src/api.rs) - REST API routes
- [docs/cms-idempotency-patterns.md](docs/cms-idempotency-patterns.md) - CMS activity patterns
- [docs/plan.md](docs/plan.md) - Full implementation plan
