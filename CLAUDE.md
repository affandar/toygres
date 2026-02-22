# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Toygres is a Rust-based control plane for hosting PostgreSQL containers as a service on Azure Kubernetes Service (AKS). It uses [Duroxide](https://github.com/microsoft/duroxide) for durable workflow orchestration.

## Build and Development Commands

```bash
# Build
cargo build --workspace              # Build all crates
cargo check --workspace              # Type-check without building

# Tests
cargo test --workspace               # Run all unit tests

# Lint
cargo clippy --workspace             # Run linter

# Local Development
./scripts/start-control-plane.sh     # Start observability + backend + UI (:8080 API, :3000 UI)
./scripts/stop-control-plane.sh      # Stop everything
./toygres server start               # Start daemon only (API on :8080)
./toygres server logs -f             # Tail server logs
./toygres server stop                # Stop local daemon

# Frontend (toygres-ui)
cd toygres-ui && npm install         # Install dependencies
cd toygres-ui && npm run dev         # Dev server on :3000
cd toygres-ui && npm run build       # Production build
cd toygres-ui && npm run lint        # ESLint

# Database
./scripts/db-init.sh                 # Initialize CMS schema
./scripts/db-migrate.sh              # Apply migrations

# AKS Deployment
./deploy/deploy-to-aks.sh            # Deploy (HTTP)
./deploy/deploy-to-aks.sh --https    # Deploy with Let's Encrypt SSL
kubectl rollout restart deployment/toygres-server -n toygres-system
```

## Architecture

### Crate Structure
- **`toygres-models`** - Shared data types (`InstanceState`, `HealthStatus`, `DeploymentConfig`)
- **`toygres-orchestrations`** - Duroxide activities and orchestrations (core business logic)
- **`toygres-server`** - Axum REST API + Duroxide worker runtime
- **`toygres-ui`** - React/TypeScript dashboard (Vite + TailwindCSS)

### Key Patterns

**Duroxide Activities** (`toygres-orchestrations/src/activities/`):
- Atomic operations wrapping K8s or CMS database operations
- Each file exports: `pub const NAME`, `pub async fn activity(ctx, input) -> Result<Output, String>`
- NAME pattern: `"toygres-orchestrations::activity::{kebab-case-name}"`
- CMS activities in `activities/cms/` subdirectory

**Duroxide Orchestrations** (`toygres-orchestrations/src/orchestrations/`):
- `create_instance` - Deploys PostgreSQL StatefulSet, waits for ready, tests connection
- `delete_instance` - Cancels instance actor, removes K8s resources
- `instance_actor` - Detached orchestration for continuous health monitoring (runs forever via continue-as-new)

**Registry** (`toygres-orchestrations/src/registry.rs`):
- All activities/orchestrations registered here
- Names defined in `names.rs`

**K8s Templates** (`toygres-orchestrations/src/templates/`):
- Tera templating for `postgres-statefulset.yaml`, `postgres-service.yaml`, `postgres-pvc.yaml`

### Database Schemas
- **`toygres_cms`** - Instance metadata (see `migrations/cms/`)
- **`toygres_duroxide`** - Managed by duroxide-pg for workflow state

## Critical Implementation Rules

### Do Not Commit/Push Without Explicit Ask

### Full-Stack Feature Completeness
Every feature must be end-to-end complete:
1. Don't add activities unless called by an orchestration
2. Don't add orchestrations unless invoked by the server API
3. Don't add API endpoints unless exposed in the UI
4. Chain: UI → API → Orchestration → Activities (if durable) or UI → API → K8s (if atomic)

### Orchestration Versioning (CRITICAL)
Orchestration code is **IMMUTABLE** once deployed. Never modify existing orchestration functions directly.

- ANY change requires a new version (even bug fixes, logging changes)
- Keep same NAME constant, create new function: `my_orch()`, `my_orch_1_0_1()`, `my_orch_1_0_2()`
- Register all versions in `registry.rs` using `register_versioned_typed()`
- Add version prefix to trace logs: `ctx.trace_info("[v1.0.2] Starting...")`
- Version upgrades happen at `continue_as_new` time, not server restart

### CMS Activity Idempotency
All CMS activities must be idempotent for Duroxide replay safety:
- **Creates**: Use `ON CONFLICT DO UPDATE` (UPSERT)
- **Updates**: Check current value first, only update if different
- **Timestamps**: Use `ctx.utcnow_ms()`, never `NOW()` or `Utc::now()`
- **State transitions**: Validate source state before transitioning
- See `docs/cms-idempotency-patterns.md` for patterns

### Before Removing "Unused" Activities
1. Search for the activity's `NAME` constant: `grep -r "activity_name" toygres-orchestrations/src/orchestrations/`
2. Check both `schedule_activity_typed` and `schedule_activity_with_retry_typed` invocations
3. **Always run `cargo build` after removals**

## Code Conventions

- **Error handling**: `anyhow::Result` in server code, `Result<T, String>` in activities/orchestrations
- **Async**: All I/O is async with Tokio
- **Tracing**: Use `tracing` macros, logs go to `~/.toygres/server.log`
- **Naming**: Activities use `"crate-name::activity::kebab-case-name"`, orchestrations use `"crate-name::orchestration::kebab-case-name"`

## Key Files

- `toygres-orchestrations/src/registry.rs` - Activity/orchestration registration
- `toygres-orchestrations/src/names.rs` - Orchestration name constants
- `toygres-server/src/api.rs` - REST API routes
- `toygres-server/src/main.rs` - RuntimeOptions configuration

## API Endpoints

REST API on `:8080` (see `toygres-server/src/api.rs`):
- `POST /api/instances` - Create instance (starts CreateInstanceOrchestration)
- `DELETE /api/instances/:name` - Delete instance
- `POST /api/instances/:name/stop|start|restart` - Instance control
- `GET /api/instances` - List instances
- `GET /api/server/orchestrations` - List Duroxide orchestrations

## Environment Variables

Required in `.env`:
- `DATABASE_URL` - PostgreSQL connection for CMS + Duroxide state
- `AKS_CLUSTER_NAME`, `AKS_RESOURCE_GROUP` - Azure/K8s configuration

## Debugging Tips

**PostgreSQL error extraction**:
```rust
if let Some(db_err) = e.as_db_error() {
    format!("severity={}, code={}, message={}",
        db_err.severity(), db_err.code().code(), db_err.message())
}
```

**Test assumptions with direct connections**:
```bash
PGPASSWORD=<password> psql "postgresql://postgres@<ip>:5432/postgres?gssencmode=disable" -c "SELECT 1"
```

**View AKS logs**:
```bash
kubectl logs -n toygres-system -l app.kubernetes.io/component=server -f
```
