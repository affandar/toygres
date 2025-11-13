# Toygres Project Context

## What is Toygres?

Toygres is a Rust-based control plane for hosting PostgreSQL containers as a service on Azure Kubernetes Service (AKS). It provides a durable, reliable way to create, manage, and monitor PostgreSQL instances.

## Architecture

### Technology Stack

- **Language**: Rust (1.85.0+)
- **Workflow Framework**: [Duroxide](https://github.com/affandar/duroxide) - Durable workflow orchestration
- **Backend Provider**: [Duroxide-PG](https://github.com/affandar/duroxide-pg) - PostgreSQL backend for Duroxide
- **Container Platform**: Azure Kubernetes Service (AKS)
- **Metadata Storage**: PostgreSQL
- **API Framework**: Axum
- **Kubernetes Client**: kube-rs

### Project Structure

```
toygres/
├── toygres-models/          # Shared data structures
├── toygres-activities/      # Duroxide activities (atomic operations)
├── toygres-orchestrations/  # Duroxide orchestrations (workflows)
└── toygres-server/          # Control plane server (API + worker)
```

### Key Concepts

**Activities**: Atomic operations that wrap Kubernetes/database operations
- `DeployPostgresActivity` - Create K8s resources for PostgreSQL
- `DeletePostgresActivity` - Remove K8s resources
- `GetInstanceStatusActivity` - Query pod status
- `HealthCheckActivity` - Verify PostgreSQL responsiveness
- `UpdateMetadataActivity` - Update metadata database
- `GenerateConnectionStringActivity` - Build connection strings

**Orchestrations**: Durable workflows coordinating activities
- `CreateInstanceOrchestration` - Full instance creation workflow
- `DeleteInstanceOrchestration` - Instance deletion workflow
- `HealthCheckOrchestration` - Continuous health monitoring (per-instance)
- `MonitorOperationOrchestration` - Query operation status

### Important Design Decisions

1. **Detached Health Checks**: When creating an instance, the CreateInstanceOrchestration starts a detached HealthCheckOrchestration. The orchestration ID is stored in metadata.

2. **Health Check Cancellation**: When deleting an instance, the DeleteInstanceOrchestration retrieves and cancels the health check orchestration before cleanup.

3. **Per-Instance Health Checks**: Each instance has its own health check orchestration that runs continuously (every 30 seconds).

4. **Infrastructure Assumptions**: 
   - AKS cluster already exists
   - Metadata database connection provided via environment variables
   - Azure credentials configured (DefaultAzureCredential)

5. **Metadata Database**: External PostgreSQL instance stores:
   - Instance metadata (state, health, connection strings)
   - Health check orchestration IDs
   - Duroxide workflow state (via duroxide-pg)

### API Endpoints

- `POST /instances` - Create PostgreSQL instance
- `DELETE /instances/{id}` - Delete instance
- `GET /instances` - List all instances
- `GET /instances/{id}` - Get instance details
- `GET /operations/{id}` - Monitor operation status
- `GET /health` - Control plane health

### Database Schema

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
```

## Development Strategy

**Incremental Approach**: We're building Toygres in phases, starting simple and adding complexity gradually.

**Current Phase**: Phase 0 - Proof of Concept
- Building basic K8s deployment code WITHOUT Duroxide first
- Validating we can deploy PostgreSQL to AKS and connect to it
- Learning K8s resource requirements and configurations

**Why This Approach?**
- Proves core functionality works before adding framework complexity
- Faster iteration on K8s configurations
- Each phase validates the previous layer
- Can compare working code when debugging Duroxide integration

## Implementation Phases (Summary)

0. **POC** - Basic K8s deployment in `examples/manual_deploy.rs`
1. **Modules** - Extract into reusable `k8s.rs` and `postgres.rs` modules
2. **Database** - Add metadata tracking with `db.rs`
3. **REST API** - Build synchronous API without Duroxide
4. **Activities** - Wrap modules in Duroxide activities
5. **Orchestrations** - Create durable workflows (without health checks)
6. **Duroxide API** - Make API asynchronous with workflows
7. **Health Checks** - Add continuous monitoring with detached orchestrations
8. **Production** - Polish, monitoring, security, deployment

See `docs/plan.md` for detailed phase descriptions.

## Development Guidelines

1. **Start Simple**: Build working code first, add Duroxide later
2. **Error Handling**: Use `anyhow::Result` for fallible operations
3. **Async**: All operations are async
4. **Testing**: Mock K8s/database clients for unit tests
5. **Logging**: Use `tracing` for structured logging
6. **Incremental**: Complete each phase before moving to the next

## References

- Full implementation plan: `docs/plan.md`
- Getting started guide: `docs/getting-started.md`
- Duroxide orchestration guide: Check the duroxide repo
- Duroxide cross-crate registry pattern: Check the duroxide repo

