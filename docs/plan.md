# Toygres Control Plane - Implementation Plan

## Overview

Toygres is a Rust-based control plane for hosting PostgreSQL containers as a service on Azure Kubernetes Service (AKS). It uses the Duroxide framework for durable workflow orchestration and PostgreSQL for metadata storage.

## Project Structure

The project is organized as a Cargo workspace with the following crates:

- **`toygres-models`**: Shared data structures (instance metadata, deployment config, health status)
- **`toygres-activities`**: Duroxide activities wrapping Azure/K8s operations
- **`toygres-orchestrations`**: Duroxide orchestrations coordinating the activities
- **`toygres-server`**: Main control plane server exposing APIs and running the Duroxide worker

## Infrastructure Setup

### Prerequisites

- Azure Kubernetes Service (AKS) cluster already provisioned
- PostgreSQL database for metadata storage
- Azure credentials configured (via environment variables or Azure CLI)

### Bootstrap Scripts

Create the following scripts to help with infrastructure setup:

- **`scripts/setup-infra.sh`**: Terraform/Azure CLI script to provision AKS cluster, networking, storage classes
- **`scripts/db-init.sh`**: Applies the initial CMS migration and prepares the Duroxide schema
- **`scripts/db-migrate.sh`**: Runs incremental CMS migrations (none yet, but keeps the pattern consistent with `duroxide-pg`)

### Environment Configuration

The control plane uses environment variables for configuration (see `.env.example`):

- `DATABASE_URL`: Connection string for metadata PostgreSQL database
- `AKS_CLUSTER_NAME`: Name of the AKS cluster
- `AKS_RESOURCE_GROUP`: Azure resource group containing the AKS cluster
- `AKS_NAMESPACE`: Kubernetes namespace for PostgreSQL deployments (default: `toygres`)
- Azure authentication via `DefaultAzureCredential`

## Core Activities (toygres-activities)

Implement Duroxide activities for atomic operations:

1. **`DeployPostgresActivity`**: Creates K8s resources (StatefulSet, PVC, Service) for a PostgreSQL pod
2. **`DeletePostgresActivity`**: Removes K8s resources for a PostgreSQL instance
3. **`GetInstanceStatusActivity`**: Queries K8s API for pod status
4. **`HealthCheckActivity`**: Connects to PostgreSQL instance and verifies it's responsive
5. **`UpdateMetadataActivity`**: Updates instance state in metadata database
6. **`GenerateConnectionStringActivity`**: Builds connection string from K8s service endpoint

Technologies:
- `kube-rs` for Kubernetes operations
- `sqlx` for database operations
- Azure SDK for Rust for Azure-specific operations (if needed)

## Orchestrations (toygres-orchestrations)

Implement durable orchestrations using the Duroxide framework:

### 1. CreateInstanceOrchestration

**Purpose**: Create a new PostgreSQL instance

**Flow**:
1. Call `DeployPostgresActivity` with name, credentials
2. Poll `GetInstanceStatusActivity` until ready
3. Call `GenerateConnectionStringActivity`
4. Call `UpdateMetadataActivity` with "running" state
5. Start detached `HealthCheckOrchestration` for this instance
6. Store health check orchestration ID in metadata
7. Return connection string

**Input**: `DeploymentConfig` (name, username, password, storage size, version)

**Output**: `CreateInstanceResponse` (instance_id, connection_string, orchestration_id)

### 2. DeleteInstanceOrchestration

**Purpose**: Delete an existing PostgreSQL instance

**Flow**:
1. Call `UpdateMetadataActivity` with "deleting" state
2. Retrieve and cancel the health check orchestration ID from metadata
3. Call `DeletePostgresActivity`
4. Call `UpdateMetadataActivity` with "deleted" state

**Input**: Instance ID

**Output**: Success/failure status

### 3. HealthCheckOrchestration

**Purpose**: Continuous health monitoring for a single PostgreSQL instance

**Flow**:
1. Input: `instance_id`
2. Loop forever:
   - Call `HealthCheckActivity` for the instance
   - Call `UpdateMetadataActivity` with health status
   - Wait 30 seconds

**Lifecycle**: Started by `CreateInstanceOrchestration`, cancelled by `DeleteInstanceOrchestration`

### 4. MonitorOperationOrchestration

**Purpose**: Query the status of any orchestration

**Flow**:
1. Query orchestration status from Duroxide
2. Return current state (pending, running, completed, failed)

**Input**: Orchestration ID

**Output**: `OperationStatus`

### Cross-Crate Registry Pattern

Use the Duroxide cross-crate registry pattern to register orchestrations and activities across the workspace.

## Control Plane Server (toygres-server)

Build the main server binary with the following components:

### 1. Configuration

- Load `.env` file using `dotenvy`
- Parse database connection string
- Validate Azure and AKS configuration

### 2. Database Client

- Initialize `sqlx` PostgreSQL pool for metadata DB
- Run migrations on startup

### 3. Duroxide Worker

- Initialize `duroxide-pg` worker connecting to metadata DB
- Register all orchestrations and activities
- Start worker loop

### 4. API Layer (REST API)

Expose the following endpoints using `axum`:

- `POST /instances` → Start `CreateInstanceOrchestration`
  - Body: `CreateInstanceRequest`
  - Response: `CreateInstanceResponse`

- `DELETE /instances/{id}` → Start `DeleteInstanceOrchestration`
  - Response: Operation status

- `GET /instances` → List all from metadata DB
  - Response: `ListInstancesResponse`

- `GET /instances/{id}` → Get single instance details
  - Response: `InstanceMetadata`

- `GET /operations/{id}` → Monitor operation status
  - Response: `OperationStatus`

- `GET /health` → Health check endpoint for the control plane itself

### 5. Health Check Scheduler

Background service that ensures all running instances have active health check orchestrations. On startup:
1. Query metadata DB for all instances in "running" state
2. Verify they have health check orchestration IDs
3. Start new health check orchestrations if missing

## Dependencies

Key Rust crates to include:

### Duroxide Framework
- `duroxide` - Core durable workflow framework
- `duroxide-pg` - PostgreSQL provider for Duroxide

### Kubernetes
- `kube` - Kubernetes client
- `k8s-openapi` - Kubernetes API types

### Azure
- `azure_core` - Azure SDK core
- `azure_identity` - Azure authentication

### Database
- `sqlx` - Async SQL with compile-time query checking
- Features: `runtime-tokio`, `postgres`, `macros`, `migrate`

### Web Framework
- `axum` - HTTP server framework
- `tower` - Middleware
- `tower-http` - HTTP middleware (tracing, CORS)

### Utilities
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `dotenvy` - Environment variable loading
- `tracing` / `tracing-subscriber` - Logging
- `anyhow` / `thiserror` - Error handling
- `chrono` - Date/time
- `uuid` - Unique identifiers

## Database Schema

### `instances` table

```sql
CREATE TYPE instance_state AS ENUM ('creating', 'running', 'deleting', 'deleted', 'failed');
CREATE TYPE health_status AS ENUM ('healthy', 'unhealthy', 'unknown');

CREATE TABLE instances (
    id UUID PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    state instance_state NOT NULL,
    health_status health_status NOT NULL DEFAULT 'unknown',
    connection_string TEXT,
    health_check_orchestration_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_instances_state ON instances(state);
CREATE INDEX idx_instances_health_status ON instances(health_status);
```

## Testing Strategy

### Unit Tests
- Test individual activities with mocked K8s/Azure clients
- Test data models and serialization
- Test configuration loading

### Integration Tests
- Test orchestrations using Duroxide in-memory provider
- Test activity coordination and error handling
- Test API endpoints with test server

### End-to-End Tests
- Test against local Kubernetes cluster (kind/minikube)
- Test full instance lifecycle (create → health checks → delete)
- Test failure scenarios and recovery

## Implementation Phases - Incremental Approach

**Philosophy**: Build working code first, then add Duroxide complexity. Validate each layer before adding the next.

### Phase 0: Proof of Concept ⭐ START HERE
**Goal**: Get basic Kubernetes/PostgreSQL deployment working without Duroxide

**Tasks**:
1. ✅ Initialize Cargo workspace with four crates
2. ✅ Define data models in `toygres-models`
3. ✅ Create database schema and migration scripts
4. ✅ Create infrastructure bootstrap scripts
5. Create `toygres-server/examples/manual_deploy.rs` that:
   - Connects to AKS cluster using kube-rs
   - Creates PostgreSQL StatefulSet, PVC, and Service
   - Waits for pod to be ready
   - Extracts connection string from Service
   - Tests connection to PostgreSQL
   - Cleans up resources

**Success Criteria**: Can deploy and connect to a PostgreSQL instance in AKS using a simple Rust binary.

**Why first?** Proves core functionality works without framework complexity. Fast iteration on K8s configurations.

---

### Phase 1: Extract Core Logic into Modules
**Goal**: Refactor POC into reusable, testable functions

**Tasks**:
1. Create `toygres-server/src/k8s.rs` module:
   - `create_postgres_resources(config) -> Result<()>`
   - `delete_postgres_resources(name) -> Result<()>`
   - `get_pod_status(name) -> Result<PodStatus>`
   - `get_service_endpoint(name) -> Result<String>`

2. Create `toygres-server/src/postgres.rs` module:
   - `test_connection(connection_string) -> Result<bool>`
   - `check_health(connection_string) -> Result<HealthStatus>`

3. Write unit tests with mocked K8s clients
4. Write integration tests against real cluster

**Success Criteria**: Clean, testable modules that can be called independently.

**Why next?** Separates concerns, enables testing, and creates reusable code for activities.

---

### Phase 2: Add Metadata Database
**Goal**: Store instance state before adding workflows

**Tasks**:
1. Run `scripts/db-init.sh` (and `scripts/db-migrate.sh`) to create schema
2. Create `toygres-server/src/db.rs` module:
   - `insert_instance(metadata) -> Result<Uuid>`
   - `update_instance_state(id, state) -> Result<()>`
   - `update_health_status(id, status) -> Result<()>`
   - `get_instance(id) -> Result<InstanceMetadata>`
   - `list_instances() -> Result<Vec<InstanceMetadata>>`

3. Create test binary that:
   - Creates instance in K8s
   - Stores metadata in database
   - Queries and updates state
   - Cleans up

**Success Criteria**: Can track instance lifecycle in database while managing K8s resources.

**Why next?** Database logic separated from workflow logic. Foundation for Duroxide state tracking.

---

### Phase 3: Simple REST API (No Duroxide Yet)
**Goal**: Working API for basic operations

**Tasks**:
1. Implement synchronous API endpoints in `toygres-server/src/api.rs`:
   - `POST /instances` - Blocks until instance ready, returns connection string
   - `DELETE /instances/{id}` - Blocks until deletion complete
   - `GET /instances` - Lists all from database
   - `GET /instances/{id}` - Gets instance details

2. Wire up modules: API → K8s module → Database module
3. Add full error handling and logging
4. Manual testing with curl

**Success Criteria**: Can create/delete instances via REST API. Everything stored in database.

**Why next?** Validates entire flow works end-to-end. Understand timing/latency requirements.

---

### Phase 4: Wrap in Duroxide Activities
**Goal**: Convert modules to Duroxide activities

**Tasks**:
1. Implement activities one by one in `toygres-activities/`:
   - `DeployPostgresActivity` (wraps `k8s::create_postgres_resources`)
   - `GetInstanceStatusActivity` (wraps `k8s::get_pod_status`)
   - `DeletePostgresActivity` (wraps `k8s::delete_postgres_resources`)
   - `HealthCheckActivity` (wraps `postgres::check_health`)
   - `UpdateMetadataActivity` (wraps `db::update_*`)
   - `GenerateConnectionStringActivity` (wraps `k8s::get_service_endpoint`)

2. Test each activity independently
3. Verify serialization/deserialization
4. Confirm error handling and retries

**Success Criteria**: All activities work independently with Duroxide.

**Why next?** We know underlying code works. Just adding Duroxide wrapper. Can test in isolation.

---

### Phase 5: Create Simple Orchestrations
**Goal**: Build durable workflows

**Tasks**:
1. Implement `CreateInstanceOrchestration` in `toygres-orchestrations/`:
   - Call activities in sequence
   - Poll for readiness
   - Return connection string
   - **DON'T** start health check yet (that's Phase 7)

2. Test with Duroxide in-memory provider
3. Verify orchestration completes, test retry behavior
4. Implement `DeleteInstanceOrchestration`:
   - Update state, call delete activity
   - **DON'T** worry about canceling health checks yet

**Success Criteria**: Can create/delete instances using durable workflows.

**Why next?** Start simple with linear workflows. Learn Duroxide patterns. Validate durability/retry.

---

### Phase 6: Add Duroxide to API
**Goal**: Make API asynchronous with durable workflows

**Tasks**:
1. Initialize Duroxide worker in `toygres-server/src/worker.rs`:
   - Connect to duroxide-pg
   - Register activities and orchestrations
   - Start worker loop

2. Update API to start orchestrations:
   - `POST /instances` → Start `CreateInstanceOrchestration`, return orchestration ID
   - `GET /operations/{id}` → Query orchestration status
   - Keep synchronous endpoints for comparison/testing

3. Test async operations and resumption after worker restart

**Success Criteria**: API starts durable workflows. Can query status. Workflows survive restarts.

**Why next?** Everything else working. Just changing API semantics. Can compare with sync version.

---

### Phase 7: Health Check Orchestrations
**Goal**: Add continuous monitoring

**Tasks**:
1. Implement `HealthCheckOrchestration` in `toygres-orchestrations/`:
   - Infinite loop with Duroxide timer
   - Call `HealthCheckActivity`
   - Update database with health status

2. Update `CreateInstanceOrchestration`:
   - Start detached health check orchestration
   - Store orchestration ID in metadata

3. Update `DeleteInstanceOrchestration`:
   - Retrieve and cancel health check orchestration ID
   - Then proceed with deletion

4. Test full lifecycle:
   - Create instance → Health checks start
   - Monitor database updates every 30s
   - Delete instance → Health checks stop

**Success Criteria**: Continuous health monitoring with automatic start/stop on create/delete.

**Why last?** Most complex feature. Depends on everything else. Involves orchestration cancellation.

---

### Phase 8: Polish & Production Readiness
**Goal**: Make it production-grade

**Tasks**:
1. Comprehensive error handling and recovery
2. Add metrics and monitoring (Prometheus?)
3. Security hardening (RBAC, secrets management)
4. Performance optimization
5. Complete documentation
6. Deployment guides and Helm charts
7. End-to-end tests against real AKS cluster

**Success Criteria**: Production-ready control plane with monitoring, docs, and deployment automation.

---

## Current Status

- ✅ Phase 0: Scaffolding complete (workspace, models, scripts, docs)
- 🔄 Phase 0: Need to implement `manual_deploy.rs` POC
- ⏳ Phase 1-8: Not started

## Next Immediate Steps

1. Implement `toygres-server/examples/manual_deploy.rs`
2. Test against real AKS cluster
3. Document learnings and K8s resource configurations
4. Move to Phase 1 when POC works reliably

