# Phase 2: Orchestrations Implementation Plan

Following the [Duroxide Cross-Crate Registry Pattern](https://github.com/affandar/duroxide/blob/main/docs/cross-crate-registry-pattern.md)

---

## Overview

Create durable orchestrations in the `toygres-orchestrations` crate that coordinate the activities from `toygres-activities` to implement the business logic for PostgreSQL instance management.

---

## Part 1: Project Structure

Organize `toygres-orchestrations/` as follows:

```
toygres-orchestrations/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # Public API
│   ├── names.rs                    # Orchestration name constants
│   ├── types.rs                    # Input/output types
│   ├── registry.rs                 # Registry builders
│   └── orchestrations/
│       ├── mod.rs
│       ├── create_instance.rs
│       ├── delete_instance.rs
│       └── health_check.rs         # (Phase 3)
```

---

## Part 2: Define Name Constants

Create `src/names.rs`:

```rust
//! Name constants for Toygres orchestrations
//!
//! Following the Duroxide naming convention: {crate-name}::{type}::{name}

/// Orchestration names
pub mod orchestrations {
    /// Create a new PostgreSQL instance
    /// 
    /// **Input:** [`crate::types::CreateInstanceInput`]  
    /// **Output:** [`crate::types::CreateInstanceOutput`]  
    /// **Activities used:**
    /// - [`toygres_activities::names::activities::DEPLOY_POSTGRES`]
    /// - [`toygres_activities::names::activities::WAIT_FOR_READY`]
    /// - [`toygres_activities::names::activities::GET_CONNECTION_STRINGS`]
    /// - [`toygres_activities::names::activities::TEST_CONNECTION`]
    /// **Duration:** ~30-60 seconds
    pub const CREATE_INSTANCE: &str = "toygres-orchestrations::orchestration::create-instance";
    
    /// Delete a PostgreSQL instance
    /// 
    /// **Input:** [`crate::types::DeleteInstanceInput`]  
    /// **Output:** [`crate::types::DeleteInstanceOutput`]  
    /// **Activities used:**
    /// - [`toygres_activities::names::activities::DELETE_POSTGRES`]
    /// **Duration:** ~10 seconds
    /// **Note:** In Phase 3, will also cancel health check orchestration
    pub const DELETE_INSTANCE: &str = "toygres-orchestrations::orchestration::delete-instance";
    
    // Phase 3: Health check orchestration
    // pub const HEALTH_CHECK: &str = "toygres-orchestrations::orchestration::health-check";
}
```

---

## Part 3: Define Types

Create `src/types.rs`:

```rust
//! Input and output types for Toygres orchestrations

use serde::{Deserialize, Serialize};

// ============================================================================
// Create Instance Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceInput {
    /// Instance name
    pub name: String,
    /// PostgreSQL password
    pub password: String,
    /// PostgreSQL version (default: "18")
    pub postgres_version: Option<String>,
    /// Storage size in GB (default: 10)
    pub storage_size_gb: Option<i32>,
    /// Use LoadBalancer for public IP (default: true)
    pub use_load_balancer: Option<bool>,
    /// DNS label for Azure DNS (optional)
    pub dns_label: Option<String>,
    /// Kubernetes namespace (default: "toygres")
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceOutput {
    /// Instance name
    pub instance_name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// IP-based connection string
    pub ip_connection_string: String,
    /// DNS-based connection string (if DNS label provided)
    pub dns_connection_string: Option<String>,
    /// External IP address
    pub external_ip: Option<String>,
    /// Azure DNS name
    pub dns_name: Option<String>,
    /// PostgreSQL version
    pub postgres_version: String,
    /// Time taken to deploy (seconds)
    pub deployment_time_seconds: u64,
}

// ============================================================================
// Delete Instance Orchestration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteInstanceInput {
    /// Instance name
    pub name: String,
    /// Kubernetes namespace (default: "toygres")
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteInstanceOutput {
    /// Instance name
    pub instance_name: String,
    /// Whether instance was deleted (false if didn't exist)
    pub deleted: bool,
}
```

---

## Part 4: Implement Orchestrations

### Orchestration 1: Create Instance

**File:** `src/orchestrations/create_instance.rs`

**Flow:**
1. Call `DEPLOY_POSTGRES` activity to create K8s resources
2. Call `WAIT_FOR_READY` activity to wait for pod
3. Call `GET_CONNECTION_STRINGS` activity to get connection strings
4. Call `TEST_CONNECTION` activity to verify PostgreSQL is responsive
5. Return connection strings and metadata

**Implementation:**

```rust
use duroxide::OrchestrationContext;
use crate::types::{CreateInstanceInput, CreateInstanceOutput};
use toygres_activities::names::activities;
use toygres_activities::types::*;

pub async fn create_instance_orchestration(
    ctx: OrchestrationContext,
    input: String,
) -> Result<String, String> {
    // Deserialize input
    let input: CreateInstanceInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info(format!("Creating PostgreSQL instance: {}", input.name));
    
    let start = std::time::Instant::now();
    let namespace = input.namespace.unwrap_or_else(|| "toygres".to_string());
    let postgres_version = input.postgres_version.unwrap_or_else(|| "18".to_string());
    let storage_size_gb = input.storage_size_gb.unwrap_or(10);
    let use_load_balancer = input.use_load_balancer.unwrap_or(true);
    
    // Step 1: Deploy PostgreSQL
    ctx.trace_info("Step 1: Deploying PostgreSQL to Kubernetes");
    let deploy_input = DeployPostgresInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
        password: input.password.clone(),
        postgres_version: postgres_version.clone(),
        storage_size_gb,
        use_load_balancer,
        dns_label: input.dns_label.clone(),
    };
    
    let deploy_result = ctx
        .schedule_activity(activities::DEPLOY_POSTGRES, serde_json::to_string(&deploy_input).unwrap())
        .into_activity()
        .await?;
    
    let _deploy_output: DeployPostgresOutput = serde_json::from_str(&deploy_result)
        .map_err(|e| format!("Failed to parse deploy output: {}", e))?;
    
    // Step 2: Wait for pod to be ready
    ctx.trace_info("Step 2: Waiting for pod to be ready");
    let wait_input = WaitForReadyInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
        timeout_seconds: 300, // 5 minutes
    };
    
    let wait_result = ctx
        .schedule_activity(activities::WAIT_FOR_READY, serde_json::to_string(&wait_input).unwrap())
        .into_activity()
        .await?;
    
    let _wait_output: WaitForReadyOutput = serde_json::from_str(&wait_result)
        .map_err(|e| format!("Failed to parse wait output: {}", e))?;
    
    // Step 3: Get connection strings
    ctx.trace_info("Step 3: Getting connection strings");
    let conn_input = GetConnectionStringsInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
        password: input.password.clone(),
        use_load_balancer,
        dns_label: input.dns_label.clone(),
    };
    
    let conn_result = ctx
        .schedule_activity(activities::GET_CONNECTION_STRINGS, serde_json::to_string(&conn_input).unwrap())
        .into_activity()
        .await?;
    
    let conn_output: GetConnectionStringsOutput = serde_json::from_str(&conn_result)
        .map_err(|e| format!("Failed to parse connection strings output: {}", e))?;
    
    // Step 4: Test connection
    ctx.trace_info("Step 4: Testing PostgreSQL connection");
    let test_input = TestConnectionInput {
        connection_string: conn_output.dns_connection_string.clone()
            .unwrap_or_else(|| conn_output.ip_connection_string.clone()),
    };
    
    let test_result = ctx
        .schedule_activity(activities::TEST_CONNECTION, serde_json::to_string(&test_input).unwrap())
        .into_activity()
        .await?;
    
    let test_output: TestConnectionOutput = serde_json::from_str(&test_result)
        .map_err(|e| format!("Failed to parse test connection output: {}", e))?;
    
    let elapsed = start.elapsed().as_secs();
    ctx.trace_info(format!("Instance created successfully in {} seconds", elapsed));
    
    // Build output
    let output = CreateInstanceOutput {
        instance_name: input.name,
        namespace,
        ip_connection_string: conn_output.ip_connection_string,
        dns_connection_string: conn_output.dns_connection_string,
        external_ip: conn_output.external_ip,
        dns_name: conn_output.dns_name,
        postgres_version: test_output.version,
        deployment_time_seconds: elapsed,
    };
    
    serde_json::to_string(&output).map_err(|e| format!("Failed to serialize output: {}", e))
}
```

### Orchestration 2: Delete Instance

**File:** `src/orchestrations/delete_instance.rs`

**Flow:**
1. Call `DELETE_POSTGRES` activity to remove K8s resources
2. Return deletion status

**Implementation:**

```rust
use duroxide::OrchestrationContext;
use crate::types::{DeleteInstanceInput, DeleteInstanceOutput};
use toygres_activities::names::activities;
use toygres_activities::types::*;

pub async fn delete_instance_orchestration(
    ctx: OrchestrationContext,
    input: String,
) -> Result<String, String> {
    // Deserialize input
    let input: DeleteInstanceInput = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid input: {}", e))?;
    
    ctx.trace_info(format!("Deleting PostgreSQL instance: {}", input.name));
    
    let namespace = input.namespace.unwrap_or_else(|| "toygres".to_string());
    
    // Step 1: Delete PostgreSQL resources
    ctx.trace_info("Step 1: Deleting PostgreSQL from Kubernetes");
    let delete_input = DeletePostgresInput {
        namespace: namespace.clone(),
        instance_name: input.name.clone(),
    };
    
    let delete_result = ctx
        .schedule_activity(activities::DELETE_POSTGRES, serde_json::to_string(&delete_input).unwrap())
        .into_activity()
        .await?;
    
    let delete_output: DeletePostgresOutput = serde_json::from_str(&delete_result)
        .map_err(|e| format!("Failed to parse delete output: {}", e))?;
    
    ctx.trace_info(format!("Instance deletion complete (deleted: {})", delete_output.deleted));
    
    // Build output
    let output = DeleteInstanceOutput {
        instance_name: input.name,
        deleted: delete_output.deleted,
    };
    
    serde_json::to_string(&output).map_err(|e| format!("Failed to serialize output: {}", e))
}
```

---

## Part 5: Registry Builder

Create `src/registry.rs`:

```rust
//! Registry builders for Toygres orchestrations

use duroxide::OrchestrationRegistry;
use crate::names::orchestrations;

/// Create an OrchestrationRegistry with all Toygres orchestrations
///
/// # Example
///
/// ```rust,no_run
/// use toygres_orchestrations::registry::create_orchestration_registry;
/// 
/// let orchestrations = create_orchestration_registry();
/// ```
pub fn create_orchestration_registry() -> OrchestrationRegistry {
    OrchestrationRegistry::builder()
        .register(
            orchestrations::CREATE_INSTANCE,
            crate::orchestrations::create_instance::create_instance_orchestration,
        )
        .register(
            orchestrations::DELETE_INSTANCE,
            crate::orchestrations::delete_instance::delete_instance_orchestration,
        )
        .build()
}
```

---

## Part 6: Public API

Update `src/lib.rs`:

```rust
//! Toygres Orchestrations - Duroxide orchestrations for PostgreSQL management
//! 
//! This crate provides durable workflows for managing PostgreSQL instances.
//! 
//! # Usage
//! 
//! ```rust,no_run
//! use toygres_orchestrations::registry::create_orchestration_registry;
//! use toygres_orchestrations::names::orchestrations;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! let orchestrations = create_orchestration_registry();
//! 
//! // Use with Duroxide runtime
//! // client.start_orchestration(
//! //     "instance-1",
//! //     orchestrations::CREATE_INSTANCE,
//! //     input_json,
//! // ).await?;
//! # Ok(())
//! # }
//! ```

pub mod names;
pub mod types;
pub mod registry;

mod orchestrations;

// Re-export key types for convenience
pub use types::*;
```

---

## Part 7: Dependencies

Update `toygres-orchestrations/Cargo.toml`:

```toml
[dependencies]
toygres-models = { path = "../toygres-models" }
toygres-activities = { path = "../toygres-activities" }

# Duroxide framework
duroxide = { workspace = true }

# Async runtime
tokio = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Error handling
anyhow = { workspace = true }

# Logging
tracing = { workspace = true }

# UUID
uuid = { workspace = true }
```

---

## Part 8: Implementation Details

### CreateInstanceOrchestration Flow

```
┌─────────────────────────────────────────┐
│ CreateInstanceOrchestration             │
├─────────────────────────────────────────┤
│ 1. DEPLOY_POSTGRES                      │
│    - Create PVC, StatefulSet, Service   │
│                                          │
│ 2. WAIT_FOR_READY                       │
│    - Poll until pod is ready            │
│    - Timeout: 5 minutes                 │
│                                          │
│ 3. GET_CONNECTION_STRINGS               │
│    - Get LoadBalancer IP                │
│    - Build IP & DNS connection strings  │
│                                          │
│ 4. TEST_CONNECTION                      │
│    - Connect to PostgreSQL              │
│    - Run SELECT version()               │
│                                          │
│ 5. Return Output                        │
│    - Connection strings                 │
│    - PostgreSQL version                 │
│    - Deployment timing                  │
└─────────────────────────────────────────┘
```

### DeleteInstanceOrchestration Flow

```
┌─────────────────────────────────────────┐
│ DeleteInstanceOrchestration             │
├─────────────────────────────────────────┤
│ 1. DELETE_POSTGRES                      │
│    - Delete Service                     │
│    - Delete StatefulSet                 │
│    - Delete PVC                         │
│                                          │
│ 2. Return Output                        │
│    - Deletion status                    │
└─────────────────────────────────────────┘
```

---

## Part 9: Error Handling

Orchestrations should handle errors gracefully:

```rust
// Activity failures propagate as orchestration errors
let result = ctx.schedule_activity(activity_name, input).await;

match result {
    Ok(output) => {
        // Continue with next step
    }
    Err(e) => {
        // Error propagates, Duroxide will retry orchestration
        ctx.trace_error(format!("Activity failed: {}", e));
        return Err(e);
    }
}
```

**Duroxide handles:**
- Automatic retries on transient failures
- State persistence across retries
- Crash recovery

---

## Part 10: Testing Strategy

### Unit Tests

Test serialization and orchestration logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_instance_input_serialization() {
        let input = CreateInstanceInput {
            name: "test-pg".to_string(),
            password: "pass123".to_string(),
            postgres_version: Some("18".to_string()),
            storage_size_gb: Some(10),
            use_load_balancer: Some(true),
            dns_label: Some("test".to_string()),
            namespace: Some("toygres".to_string()),
        };
        
        let json = serde_json::to_string(&input).unwrap();
        let parsed: CreateInstanceInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input.name, parsed.name);
    }
    
    #[test]
    fn test_create_instance_output_serialization() {
        let output = CreateInstanceOutput {
            instance_name: "test-pg".to_string(),
            namespace: "toygres".to_string(),
            ip_connection_string: "postgresql://...".to_string(),
            dns_connection_string: None,
            external_ip: Some("1.2.3.4".to_string()),
            dns_name: None,
            postgres_version: "PostgreSQL 18.0".to_string(),
            deployment_time_seconds: 45,
        };
        
        let json = serde_json::to_string(&output).unwrap();
        let parsed: CreateInstanceOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output.instance_name, parsed.instance_name);
    }
}
```

### Integration Tests

Test orchestrations with Duroxide in-memory provider:

```rust
// tests/orchestration_tests.rs

use std::sync::Arc;
use duroxide::{Runtime, Client};
use duroxide_pg::SqliteProvider;

#[tokio::test]
async fn test_create_instance_orchestration() {
    // Setup in-memory Duroxide runtime
    let store = Arc::new(SqliteProvider::new_in_memory().await.unwrap());
    let activities = Arc::new(toygres_activities::registry::create_activity_registry());
    let orchestrations = toygres_orchestrations::registry::create_orchestration_registry();
    
    let runtime = Runtime::start_with_store(
        store.clone(),
        activities,
        orchestrations,
    ).await;
    
    let client = Client::new(store);
    
    // Start orchestration
    let input = toygres_orchestrations::types::CreateInstanceInput {
        name: "test-instance".to_string(),
        password: "testpass".to_string(),
        postgres_version: Some("18".to_string()),
        storage_size_gb: Some(10),
        use_load_balancer: Some(false), // Use ClusterIP for testing
        dns_label: None,
        namespace: Some("toygres".to_string()),
    };
    
    client.start_orchestration(
        "test-1",
        toygres_orchestrations::names::orchestrations::CREATE_INSTANCE,
        serde_json::to_string(&input).unwrap(),
    ).await.unwrap();
    
    // Wait for completion
    let status = client.wait_for_orchestration(
        "test-1",
        std::time::Duration::from_secs(120),
    ).await.unwrap();
    
    // Verify
    assert!(matches!(status, duroxide::OrchestrationStatus::Completed { .. }));
    
    runtime.shutdown(None).await;
}
```

---

## Part 11: Key Duroxide Concepts

### Determinism Rules

Orchestrations MUST be deterministic:

✅ **Allowed:**
- Call activities via `ctx.schedule_activity()`
- Use Duroxide timers: `ctx.create_timer(duration).await`
- Deterministic logic (if/else based on inputs)
- Serialize/deserialize data

❌ **NOT Allowed:**
- Direct I/O operations (use activities instead)
- `tokio::sleep` (use `ctx.create_timer()`)
- Random numbers (unless seeded deterministically)
- System time (use `ctx.current_time()`)

### Activity Calls

```rust
// Schedule an activity
let result = ctx
    .schedule_activity(ACTIVITY_NAME, input_json)
    .into_activity()  // Convert to activity future
    .await?;          // Wait for completion

// Parse result
let output: MyOutput = serde_json::from_str(&result)?;
```

### Error Propagation

```rust
// Errors propagate and trigger orchestration retry
ctx.schedule_activity(ACTIVITY_NAME, input)
    .into_activity()
    .await?;  // ? propagates error, Duroxide retries orchestration

// Custom error messages
.await.map_err(|e| format!("Deploy failed: {}", e))?;
```

---

## Part 12: Phase 3 Extensions (Future)

### Health Check Orchestration (Deferred)

**Will be added in Phase 3:**

```rust
/// Continuous health monitoring for a single instance
/// 
/// **Input:** [`crate::types::HealthCheckInput`]  
/// **Output:** Never completes (runs forever)  
/// **Activities used:**
/// - [`toygres_activities::names::activities::TEST_CONNECTION`]
/// - [`toygres_activities::names::activities::UPDATE_METADATA`]
/// **Lifecycle:** Started by CreateInstanceOrchestration, cancelled by DeleteInstanceOrchestration
pub const HEALTH_CHECK: &str = "toygres-orchestrations::orchestration::health-check";
```

**Implementation sketch:**
```rust
pub async fn health_check_orchestration(
    ctx: OrchestrationContext,
    input: String,
) -> Result<String, String> {
    // Loop forever
    loop {
        // Test connection
        // Update metadata with health status
        // Wait 30 seconds (using Duroxide timer)
        ctx.create_timer(Duration::from_secs(30)).await?;
    }
}
```

---

## Implementation Steps

### Step 1: Setup Structure
1. Create `names.rs` with orchestration name constants
2. Create `types.rs` with input/output structs
3. Update `Cargo.toml` with dependencies
4. Create `orchestrations/mod.rs`

### Step 2: Implement CreateInstanceOrchestration
1. Create `orchestrations/create_instance.rs`
2. Implement the 4-step flow
3. Add error handling
4. Add trace logging

### Step 3: Implement DeleteInstanceOrchestration
1. Create `orchestrations/delete_instance.rs`
2. Call DELETE_POSTGRES activity
3. Add error handling

### Step 4: Create Registry
1. Implement `registry.rs` with `create_orchestration_registry()`
2. Register both orchestrations

### Step 5: Update Public API
1. Update `lib.rs` with module exports
2. Add documentation

### Step 6: Testing
1. Unit tests for serialization
2. Integration tests with in-memory Duroxide
3. Document orchestrations

---

## Success Criteria

When Phase 2 is complete:

✅ 2 orchestrations implemented in `toygres-orchestrations` crate  
✅ Orchestration names follow `toygres-orchestrations::orchestration::{name}` pattern  
✅ All types strongly-typed with serde  
✅ Registry builder returns complete `OrchestrationRegistry`  
✅ CreateInstance orchestration coordinates 4 activities  
✅ DeleteInstance orchestration handles cleanup  
✅ All orchestrations are deterministic  
✅ Unit tests for serialization  
✅ Integration tests with in-memory Duroxide  
✅ Documentation for all orchestrations  

**Deferred to Phase 3:**
- Health check orchestration (requires metadata database and detached orchestrations)
- Metadata tracking activities integration

---

## Dependencies Between Phases

```
Phase 1 (Activities) ──→ Phase 2 (Orchestrations) ──→ Phase 3 (Server + API)
                                    ↓
                            Phase 4 (Health Checks)
```

- **Phase 2 depends on Phase 1**: Uses activities from `toygres-activities`
- **Phase 3 depends on Phase 2**: Server starts orchestrations
- **Phase 4 extends Phase 2**: Adds health check orchestration

---

## Next: Phase 3 (Control Plane Server)

After orchestrations are complete, we'll integrate them into `toygres-server`:
- Initialize Duroxide worker
- Start orchestrations via API
- Query orchestration status
- Build REST API endpoints

But that's for later! Let's focus on orchestrations first.

---

## Questions Before Starting

1. Should we start with just CreateInstance orchestration first?
2. Do you want to test orchestrations with in-memory Duroxide before moving to Phase 3?
3. Should we add any retry logic or timeout configuration?

