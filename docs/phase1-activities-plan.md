# Phase 1: Activities Implementation Plan

Following the [Duroxide Cross-Crate Registry Pattern](https://github.com/affandar/duroxide/blob/main/docs/cross-crate-registry-pattern.md)

---

## Overview

Extract the working code from `manual_deploy.rs` (Phase 0) into Duroxide activities in the `toygres-activities` crate. This follows the standard pattern for building reusable Duroxide workflows.

---

## Part 1: Project Structure

Organize `toygres-activities/` as follows:

```
toygres-activities/
├── Cargo.toml
├── src/
│   ├── lib.rs                   # Public API
│   ├── names.rs                 # Name constants (following naming convention)
│   ├── types.rs                 # Input/output types (move from toygres-models)
│   ├── registry.rs              # Registry builders
│   ├── k8s_client.rs            # Shared K8s client wrapper
│   ├── activities/
│   │   ├── mod.rs
│   │   ├── deploy_postgres.rs
│   │   ├── delete_postgres.rs
│   │   ├── wait_for_ready.rs
│   │   ├── get_connection_strings.rs
│   │   ├── test_connection.rs
│   │   └── update_metadata.rs
│   └── templates/
│       ├── postgres-pvc.yaml
│       ├── postgres-statefulset.yaml
│       └── postgres-service.yaml
```

---

## Part 2: Define Name Constants

Create `src/names.rs`:

```rust
//! Name constants for Toygres activities
//!
//! Following the Duroxide naming convention: {crate-name}::{type}::{name}

/// Activity names
pub mod activities {
    /// Deploy PostgreSQL to Kubernetes
    /// 
    /// **Input:** [`crate::types::DeployPostgresInput`]  
    /// **Output:** [`crate::types::DeployPostgresOutput`]  
    /// **Idempotent:** Yes (checks if resources exist)
    /// **Operations:**
    /// - Creates PersistentVolumeClaim
    /// - Creates StatefulSet
    /// - Creates Service (LoadBalancer or ClusterIP)
    pub const DEPLOY_POSTGRES: &str = "toygres-activities::activity::deploy-postgres";
    
    /// Delete PostgreSQL deployment from Kubernetes
    /// 
    /// **Input:** [`crate::types::DeletePostgresInput`]  
    /// **Output:** [`crate::types::DeletePostgresOutput`]  
    /// **Idempotent:** Yes (no-op if already deleted)
    /// **Operations:**
    /// - Deletes Service
    /// - Deletes StatefulSet
    /// - Deletes PersistentVolumeClaim
    pub const DELETE_POSTGRES: &str = "toygres-activities::activity::delete-postgres";
    
    /// Wait for PostgreSQL pod to be ready
    /// 
    /// **Input:** [`crate::types::WaitForReadyInput`]  
    /// **Output:** [`crate::types::WaitForReadyOutput`]  
    /// **Idempotent:** Yes (returns immediately if already ready)
    /// **Operations:**
    /// - Polls pod status until Ready condition is True
    /// - Timeout after configured duration
    pub const WAIT_FOR_READY: &str = "toygres-activities::activity::wait-for-ready";
    
    /// Get connection strings for PostgreSQL instance
    /// 
    /// **Input:** [`crate::types::GetConnectionStringsInput`]  
    /// **Output:** [`crate::types::GetConnectionStringsOutput`]  
    /// **Idempotent:** Yes
    /// **Operations:**
    /// - Gets LoadBalancer external IP
    /// - Constructs IP-based connection string
    /// - Constructs DNS-based connection string (if DNS label provided)
    pub const GET_CONNECTION_STRINGS: &str = "toygres-activities::activity::get-connection-strings";
    
    /// Test PostgreSQL connection
    /// 
    /// **Input:** [`crate::types::TestConnectionInput`]  
    /// **Output:** [`crate::types::TestConnectionOutput`]  
    /// **Idempotent:** Yes
    /// **Operations:**
    /// - Connects to PostgreSQL
    /// - Runs SELECT version() query
    /// - Returns version string
    pub const TEST_CONNECTION: &str = "toygres-activities::activity::test-connection";
    
    // Note: UPDATE_METADATA activity will be added in Phase 2
    // when we implement the metadata database layer
}
```

**Key Points:**
- Use `toygres-activities` as the crate prefix
- Use `::activity::` for activities
- Use kebab-case names (e.g., `deploy-postgres`)
- Document input/output types, idempotency, and operations

---

## Part 3: Define Types

Move types from `toygres-models` to `toygres-activities/src/types.rs`:

```rust
//! Input and output types for Toygres activities

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Deploy PostgreSQL Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPostgresInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name (used for K8s resource names)
    pub instance_name: String,
    /// PostgreSQL password
    pub password: String,
    /// PostgreSQL version (e.g., "16", "18")
    pub postgres_version: String,
    /// Storage size in GB
    pub storage_size_gb: i32,
    /// Use LoadBalancer (true) or ClusterIP (false)
    pub use_load_balancer: bool,
    /// Optional DNS label for Azure DNS (<label>.<region>.cloudapp.azure.com)
    pub dns_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPostgresOutput {
    /// Instance name
    pub instance_name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// Whether resources were created (false if already existed)
    pub created: bool,
}

// ============================================================================
// Delete PostgreSQL Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePostgresInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePostgresOutput {
    /// Whether resources were deleted (false if didn't exist)
    pub deleted: bool,
}

// ============================================================================
// Wait For Ready Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForReadyInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
    /// Timeout in seconds
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForReadyOutput {
    /// Pod phase (e.g., "Running")
    pub pod_phase: String,
    /// Time taken to become ready (seconds)
    pub ready_after_seconds: u64,
}

// ============================================================================
// Get Connection Strings Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConnectionStringsInput {
    /// Kubernetes namespace
    pub namespace: String,
    /// Instance name
    pub instance_name: String,
    /// PostgreSQL password
    pub password: String,
    /// Whether LoadBalancer was used
    pub use_load_balancer: bool,
    /// DNS label (if used)
    pub dns_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConnectionStringsOutput {
    /// IP-based connection string
    pub ip_connection_string: String,
    /// DNS-based connection string (if DNS label provided)
    pub dns_connection_string: Option<String>,
    /// External IP address (if LoadBalancer)
    pub external_ip: Option<String>,
    /// Azure DNS name (if DNS label provided)
    pub dns_name: Option<String>,
}

// ============================================================================
// Test Connection Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionInput {
    /// Connection string to test
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionOutput {
    /// PostgreSQL version string
    pub version: String,
    /// Whether connection succeeded
    pub connected: bool,
}

// ============================================================================
// Update Metadata Activity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadataInput {
    /// Instance ID (UUID)
    pub instance_id: Uuid,
    /// Instance name
    pub instance_name: String,
    /// Current state
    pub state: String,
    /// Health status
    pub health_status: Option<String>,
    /// IP connection string
    pub ip_connection_string: Option<String>,
    /// DNS connection string
    pub dns_connection_string: Option<String>,
    /// Health check orchestration ID
    pub health_check_orchestration_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadataOutput {
    /// Instance ID
    pub instance_id: Uuid,
    /// Whether record was created (true) or updated (false)
    pub created: bool,
}
```

**Key Points:**
- All types use `Serialize` and `Deserialize`
- Clear, descriptive field names
- Doc comments on all fields
- Use `Option<T>` for optional fields

---

## Part 4: Implement Activities

### Activity 1: Deploy PostgreSQL

**File:** `src/activities/deploy_postgres.rs`

**What to Extract from `manual_deploy.rs`:**
- `create_postgres_instance()` function
- YAML template loading and rendering
- K8s resource creation (PVC, StatefulSet, Service)

**Implementation Structure:**
```rust
use duroxide::ActivityContext;
use crate::types::{DeployPostgresInput, DeployPostgresOutput};

pub async fn deploy_postgres_activity(
    ctx: ActivityContext,
    input: String,
) -> Result<String, String> {
    // 1. Deserialize input
    let input: DeployPostgresInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info(format!("Deploying PostgreSQL: {}", input.instance_name));
    
    // 2. Get K8s client
    let client = kube::Client::try_default().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // 3. Check idempotency - do resources already exist?
    let already_exists = check_resources_exist(&client, &input.namespace, &input.instance_name).await?;
    
    if already_exists {
        ctx.trace_info("Resources already exist, skipping creation");
        return Ok(serde_json::to_string(&DeployPostgresOutput {
            instance_name: input.instance_name,
            namespace: input.namespace,
            created: false,
        }).unwrap());
    }
    
    // 4. Create resources using templates
    create_k8s_resources(&client, &input, &ctx).await?;
    
    ctx.trace_info("PostgreSQL deployment complete");
    
    // 5. Serialize output
    let output = DeployPostgresOutput {
        instance_name: input.instance_name,
        namespace: input.namespace,
        created: true,
    };
    
    serde_json::to_string(&output).map_err(|e| format!("Failed to serialize output: {}", e))
}
```

**Key Implementation Details:**
- Extract template loading logic from Phase 0
- Make idempotent: check if StatefulSet exists before creating
- Use `ctx.trace_info()` for logging
- Handle all errors gracefully

### Activity 2: Delete PostgreSQL

**File:** `src/activities/delete_postgres.rs`

**What to Extract from `manual_deploy.rs`:**
- `cleanup_postgres_instance()` function
- K8s resource deletion logic

**Implementation:**
```rust
pub async fn delete_postgres_activity(
    ctx: ActivityContext,
    input: String,
) -> Result<String, String> {
    let input: DeletePostgresInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info(format!("Deleting PostgreSQL: {}", input.instance_name));
    
    let client = kube::Client::try_default().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // Idempotent: Check if resources exist
    let exists = check_resources_exist(&client, &input.namespace, &input.instance_name).await?;
    
    if !exists {
        ctx.trace_info("Resources don't exist, nothing to delete");
        return Ok(serde_json::to_string(&DeletePostgresOutput { deleted: false }).unwrap());
    }
    
    // Delete in order: Service -> StatefulSet -> PVC
    delete_k8s_resources(&client, &input.namespace, &input.instance_name, &ctx).await?;
    
    ctx.trace_info("PostgreSQL deletion complete");
    
    Ok(serde_json::to_string(&DeletePostgresOutput { deleted: true }).unwrap())
}
```

### Activity 3: Wait For Ready

**File:** `src/activities/wait_for_ready.rs`

**What to Extract:**
- `wait_for_pod_ready()` function
- Pod status polling logic

**Implementation:**
```rust
pub async fn wait_for_ready_activity(
    ctx: ActivityContext,
    input: String,
) -> Result<String, String> {
    let input: WaitForReadyInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info(format!("Waiting for pod to be ready: {}", input.instance_name));
    
    let client = kube::Client::try_default().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(input.timeout_seconds);
    
    // Poll pod status
    let result = poll_pod_ready(&client, &input.namespace, &input.instance_name, timeout, &ctx).await?;
    
    let elapsed = start.elapsed().as_secs();
    ctx.trace_info(format!("Pod ready after {} seconds", elapsed));
    
    Ok(serde_json::to_string(&WaitForReadyOutput {
        pod_phase: result.phase,
        ready_after_seconds: elapsed,
    }).unwrap())
}
```

### Activity 4: Get Connection Strings

**File:** `src/activities/get_connection_strings.rs`

**What to Extract:**
- `get_connection_strings()` function
- LoadBalancer IP waiting logic
- Azure region detection
- DNS name construction

**Implementation:**
```rust
pub async fn get_connection_strings_activity(
    ctx: ActivityContext,
    input: String,
) -> Result<String, String> {
    let input: GetConnectionStringsInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info(format!("Getting connection strings for: {}", input.instance_name));
    
    let client = kube::Client::try_default().await
        .map_err(|e| format!("Failed to create K8s client: {}", e))?;
    
    // Get IP and construct connection strings
    let (ip_conn, dns_conn, external_ip, dns_name) = build_connection_strings(
        &client,
        &input,
        &ctx,
    ).await?;
    
    ctx.trace_info("Connection strings generated");
    
    Ok(serde_json::to_string(&GetConnectionStringsOutput {
        ip_connection_string: ip_conn,
        dns_connection_string: dns_conn,
        external_ip,
        dns_name,
    }).unwrap())
}
```

### Activity 5: Test Connection

**File:** `src/activities/test_connection.rs`

**What to Extract:**
- `test_postgres_connection()` function
- PostgreSQL client connection logic

**Implementation:**
```rust
pub async fn test_connection_activity(
    ctx: ActivityContext,
    input: String,
) -> Result<String, String> {
    let input: TestConnectionInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info("Testing PostgreSQL connection");
    
    // Connect and query version
    let version = connect_and_query_version(&input.connection_string, &ctx).await?;
    
    ctx.trace_info(format!("Connected successfully, version: {}", version));
    
    Ok(serde_json::to_string(&TestConnectionOutput {
        version,
        connected: true,
    }).unwrap())
}
```

### Activity 6: Update Metadata (Deferred to Phase 2)

**Status:** Not implemented in Phase 1

This activity will be added in Phase 2 when we implement the metadata database layer. It will handle storing instance state, connection strings, and health status in the PostgreSQL metadata database.

---

## Part 5: Registry Builder

Create `src/registry.rs`:

```rust
//! Registry builders for Toygres activities

use duroxide::runtime::registry::ActivityRegistry;
use crate::names::activities;

/// Create an ActivityRegistry with all Toygres activities
///
/// # Example
///
/// ```rust
/// let activities = toygres_activities::registry::create_activity_registry();
/// ```
pub fn create_activity_registry() -> ActivityRegistry {
    ActivityRegistry::builder()
        .register(
            activities::DEPLOY_POSTGRES,
            crate::activities::deploy_postgres::deploy_postgres_activity,
        )
        .register(
            activities::DELETE_POSTGRES,
            crate::activities::delete_postgres::delete_postgres_activity,
        )
        .register(
            activities::WAIT_FOR_READY,
            crate::activities::wait_for_ready::wait_for_ready_activity,
        )
        .register(
            activities::GET_CONNECTION_STRINGS,
            crate::activities::get_connection_strings::get_connection_strings_activity,
        )
        .register(
            activities::TEST_CONNECTION,
            crate::activities::test_connection::test_connection_activity,
        )
        // Note: UPDATE_METADATA will be added in Phase 2
        .build()
}
```

---

## Part 6: Shared K8s Client Wrapper

Create `src/k8s_client.rs` for shared K8s operations:

```rust
//! Shared Kubernetes client utilities

use anyhow::Result;
use kube::Client;

/// Get or create a Kubernetes client
/// 
/// This will be shared across activities to avoid creating multiple clients
pub async fn get_k8s_client() -> Result<Client> {
    Client::try_default().await
        .map_err(|e| anyhow::anyhow!("Failed to create K8s client: {}", e))
}

/// Check if PostgreSQL resources exist
pub async fn check_resources_exist(
    client: &Client,
    namespace: &str,
    instance_name: &str,
) -> Result<bool> {
    use k8s_openapi::api::apps::v1::StatefulSet;
    use kube::api::Api;
    
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    
    match statefulsets.get(instance_name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(response)) if response.code == 404 => Ok(false),
        Err(e) => Err(anyhow::anyhow!("Failed to check StatefulSet: {}", e)),
    }
}

// ... more shared utilities
```

---

## Part 7: Public API

Update `src/lib.rs`:

```rust
//! Toygres Activities - Duroxide activities for PostgreSQL management
//! 
//! This crate provides reusable activities for deploying and managing
//! PostgreSQL instances on Kubernetes.
//! 
//! # Usage
//! 
//! ```rust
//! use toygres_activities::registry::create_activity_registry;
//! use toygres_activities::names::activities;
//! 
//! let activities = create_activity_registry();
//! 
//! // Use in orchestrations
//! ctx.schedule_activity(activities::DEPLOY_POSTGRES, input_json).await?;
//! ```

pub mod names;
pub mod types;
pub mod registry;
pub mod k8s_client;

mod activities {
    pub mod deploy_postgres;
    pub mod delete_postgres;
    pub mod wait_for_ready;
    pub mod get_connection_strings;
    pub mod test_connection;
    // update_metadata will be added in Phase 2
}

// Re-export key types for convenience
pub use types::*;
```

---

## Part 8: Dependencies Update

Update `toygres-activities/Cargo.toml`:

```toml
[dependencies]
# Duroxide framework
duroxide = { workspace = true }

# Async runtime
tokio = { workspace = true }

# Kubernetes
kube = { workspace = true }
k8s-openapi = { workspace = true }

# Database
sqlx = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
serde_yaml = "0.9"

# Templating
tera = "1.19"

# PostgreSQL client
tokio-postgres = "0.7"

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }

# UUID
uuid = { workspace = true }

# Time
chrono = { workspace = true }
```

---

## Part 9: Move Templates

Move YAML templates from `toygres-server/templates/` to `toygres-activities/src/templates/`:

```
toygres-activities/src/templates/
├── postgres-pvc.yaml
├── postgres-statefulset.yaml
└── postgres-service.yaml
```

Update the `include_str!()` paths in the activity implementations.

---

## Part 10: Testing Strategy

### Unit Tests (per activity)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_deploy_postgres_serialization() {
        let input = DeployPostgresInput {
            namespace: "test".to_string(),
            instance_name: "test-pg".to_string(),
            // ...
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: DeployPostgresInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input.instance_name, parsed.instance_name);
    }
    
    // TODO: Add integration tests with mock K8s client
}
```

### Integration Tests

Test activities using Duroxide in-memory provider:

```rust
// tests/activity_tests.rs

#[tokio::test]
async fn test_deploy_and_delete_flow() {
    let activities = toygres_activities::registry::create_activity_registry();
    
    // Create in-memory Duroxide runtime
    // Call activities in sequence
    // Verify outputs
}
```

---

## Implementation Steps

### Step 1: Setup Structure
1. Create `names.rs` with all activity name constants
2. Create or move `types.rs` with all input/output structs
3. Update `Cargo.toml` with dependencies
4. Update `lib.rs` with module structure

### Step 2: Extract and Refactor Deploy Activity
1. Copy `create_postgres_instance()` from `manual_deploy.rs`
2. Add idempotency check
3. Wrap in activity signature
4. Add serialization/deserialization
5. Move templates to activities crate

### Step 3: Extract and Refactor Delete Activity
1. Copy `cleanup_postgres_instance()` from `manual_deploy.rs`
2. Add idempotency check
3. Wrap in activity signature

### Step 4: Extract and Refactor Wait Activity
1. Copy `wait_for_pod_ready()` from `manual_deploy.rs`
2. Add timeout parameter
3. Return timing information

### Step 5: Extract and Refactor Connection Strings Activity
1. Copy `get_connection_strings()` from `manual_deploy.rs`
2. Copy `get_azure_region()` helper
3. Handle all connection string variations

### Step 6: Extract and Refactor Test Connection Activity
1. Copy `test_postgres_connection()` from `manual_deploy.rs`
2. Add retry logic (PostgreSQL might not be immediately ready)

### Step 7: Create Registry
1. Implement `create_activity_registry()` in `registry.rs`
2. Register all activities
3. Export from `lib.rs`

### Step 8: Create Shared Utilities
1. Implement `k8s_client.rs` with shared functions
2. Template loading utilities
3. Error conversion helpers

### Step 9: Testing
1. Unit tests for serialization
2. Unit tests for idempotency logic
3. Integration tests (with mock K8s if possible)
4. Document all activities

---

## Key Design Principles

### 1. Idempotency

All activities MUST be idempotent:
- **Deploy**: Check if resources exist, skip if already created
- **Delete**: No-op if resources don't exist
- **Wait**: Return immediately if already ready
- **Get Strings**: Query current state (always safe)
- **Test Connection**: Always safe to re-test
- **Update Metadata**: Use upsert (INSERT ... ON CONFLICT UPDATE)

### 2. Error Handling

Activities return `Result<String, String>`:
- Success: Serialized output struct
- Error: Human-readable error message (can include serialized error struct)

### 3. Logging

Use `ActivityContext.trace_*()` methods:
```rust
ctx.trace_info("Starting operation");
ctx.trace_warn("Potential issue detected");
ctx.trace_error("Operation failed");
```

This provides automatic correlation IDs and structured logging.

### 4. Configuration

Activities get configuration from:
1. **Input structs** - Operation-specific parameters
2. **Environment variables** - Global config (DATABASE_URL, etc.)
3. **ActivityContext** - Runtime context from Duroxide

---

## Migration from Phase 0

### Code Reuse

Most of the code from `manual_deploy.rs` can be reused:
- ✅ Template rendering logic
- ✅ K8s resource creation/deletion
- ✅ Pod status polling
- ✅ Connection string construction
- ✅ PostgreSQL connection testing

### Changes Needed

1. **Signatures**: Wrap in activity signature (ActivityContext, String in/out)
2. **Serialization**: Add JSON serialization/deserialization
3. **Idempotency**: Add checks before operations
4. **Logging**: Replace `info!()` with `ctx.trace_info()`
5. **Error Handling**: Return `Result<String, String>`

### What Stays in Examples

Keep `manual_deploy.rs` as a reference:
- Shows how to call activities directly
- Useful for testing activities outside orchestrations
- Documents the end-to-end flow

---

## Success Criteria

When Phase 1 is complete:

✅ 5 core activities implemented in `toygres-activities` crate (metadata deferred to Phase 2)
✅ Activity names follow `toygres-activities::activity::{name}` pattern  
✅ All types strongly-typed with serde  
✅ Registry builder returns complete `ActivityRegistry`  
✅ All activities are idempotent  
✅ Templates moved to activities crate  
✅ Shared utilities extracted to `k8s_client.rs`  
✅ Unit tests for serialization  
✅ Documentation for all activities  
✅ `manual_deploy.rs` still works (as a reference)  

**Deferred to Phase 2:**
- `update-metadata` activity (requires metadata database setup)  

---

## Next: Phase 2 (Orchestrations)

After activities are complete, we'll create orchestrations in `toygres-orchestrations/` that:
- Call activities in sequence
- Handle retries and errors
- Implement the business logic
- Follow the same registry pattern

But that's for later! Let's focus on activities first.

---

## Questions Before Starting

1. Should we start with just one activity (e.g., Deploy) to validate the pattern?
2. Do you want to keep `manual_deploy.rs` as-is for reference, or migrate it to use the activities?
3. Should we add any additional activities beyond the 6 listed above?

