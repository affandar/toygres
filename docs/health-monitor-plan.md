# Health Monitor Orchestration Plan

## Overview

Implement a continuous health monitoring orchestration that runs per instance, checking PostgreSQL health every 30 seconds. The orchestration starts when an instance is created and is cancelled when the instance is deleted.

---

## Architecture

### Orchestration: `health_monitor`

**Purpose**: Continuously monitor a single PostgreSQL instance's health

**Lifecycle**:
- **Started by**: `create_instance` orchestration using `start_orchestration_detached()`
- **Runs**: Detached from parent (not a sub-orchestration)
- **Restart**: Uses continue-as-new pattern after each 30s iteration
- **Stopped by**: `delete_instance` orchestration cancels it, OR it detects deleted/deleting state and exits
- **Duration**: Runs indefinitely (one 30s iteration at a time)

**Flow**:
```
1. Get instance details from CMS (connection string, state)
2. Check if instance is deleted/deleting → exit if so
3. Test PostgreSQL connection
4. Record health check result in CMS
5. Update instance health_status in CMS
6. Wait 30 seconds (using Duroxide timer)
7. Continue as new (restart orchestration to prevent unbounded history)
```

**Why Continue-as-New?**
- Prevents unbounded history growth in Duroxide
- Each iteration is a fresh orchestration execution with new history
- Maintains determinism and replay safety
- Calls `ctx.continue_as_new(input)` which ends current instance and starts new one
- Keeps the same orchestration ID across restarts
- More efficient than infinite loops for long-running orchestrations
- Required pattern for eternal/recurring orchestrations

---

## Components Needed

### 1. New Orchestration Types

**File**: `toygres-orchestrations/src/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthMonitorInput {
    /// K8s instance name (with GUID)
    pub k8s_name: String,
    /// Kubernetes namespace
    pub namespace: String,
    /// Orchestration ID
    pub orchestration_id: String,
}

// Output: Returns Result<(), String>
// - On graceful exit (deleted/deleting): Returns Err to stop monitoring
// - On continue-as-new: Calls ctx.continue_as_new() which never returns
// - The orchestration restarts with fresh history after continue_as_new
```

### 2. New CMS Activities

#### Activity: `record_health_check`

**Purpose**: Insert health check result into `instance_health_checks` table

**Input**:
```rust
pub struct RecordHealthCheckInput {
    pub k8s_name: String,
    pub status: String,  // "healthy", "unhealthy", "unknown"
    pub postgres_version: Option<String>,
    pub response_time_ms: Option<i32>,
    pub error_message: Option<String>,
}
```

**Output**:
```rust
pub struct RecordHealthCheckOutput {
    pub recorded: bool,
    pub check_id: i64,
}
```

**Database Operation**:
```sql
INSERT INTO toygres_cms.instance_health_checks 
(instance_id, status, postgres_version, response_time_ms, error_message, checked_at)
SELECT i.id, $2, $3, $4, $5, NOW()
FROM toygres_cms.instances i
WHERE i.k8s_name = $1
RETURNING id;
```

#### Activity: `update_instance_health`

**Purpose**: Update instance's current `health_status` field

**Input**:
```rust
pub struct UpdateInstanceHealthInput {
    pub k8s_name: String,
    pub health_status: String,  // "healthy", "unhealthy", "unknown"
}
```

**Output**:
```rust
pub struct UpdateInstanceHealthOutput {
    pub updated: bool,
}
```

**Database Operation**:
```sql
UPDATE toygres_cms.instances
SET health_status = $2, updated_at = NOW()
WHERE k8s_name = $1
  AND state = 'running'
RETURNING id;
```

#### Activity: `get_instance_connection`

**Purpose**: Retrieve connection string for health check

**Input**:
```rust
pub struct GetInstanceConnectionInput {
    pub k8s_name: String,
}
```

**Output**:
```rust
pub struct GetInstanceConnectionOutput {
    pub found: bool,
    pub connection_string: Option<String>,
    pub state: Option<String>,
}
```

**Database Operation**:
```sql
SELECT 
    COALESCE(dns_connection_string, ip_connection_string) as connection_string,
    state::text
FROM toygres_cms.instances
WHERE k8s_name = $1
LIMIT 1;
```

---

## 3. Orchestration Implementation

**File**: `toygres-orchestrations/src/orchestrations/health_monitor.rs`

```rust
pub async fn health_monitor_orchestration(
    ctx: OrchestrationContext,
    input: HealthMonitorInput,
) -> Result<(), String> {
    ctx.trace_info(format!(
        "Health monitor iteration for instance: {} (orchestration: {})",
        input.k8s_name, input.orchestration_id
    ));
        
        // Step 1: Get instance connection string from CMS
        let conn_info = ctx
            .schedule_activity_typed::<GetInstanceConnectionInput, GetInstanceConnectionOutput>(
                activities::cms::GET_INSTANCE_CONNECTION,
                &GetInstanceConnectionInput {
                    k8s_name: input.k8s_name.clone(),
                },
            )
            .into_activity_typed::<GetInstanceConnectionOutput>()
            .await
            .map_err(|e| format!("Failed to get instance connection: {}", e))?;
        
    // Step 2: Check if instance still exists and is not being deleted
    if !conn_info.found {
        ctx.trace_warn("Instance no longer exists in CMS, stopping health monitor");
        return Err("Instance not found, stopping health monitor".to_string());
    }
    
    if let Some(state) = &conn_info.state {
        if state == "deleted" || state == "deleting" {
            ctx.trace_info("Instance is being deleted, stopping health monitor");
            return Err("Instance is deleted/deleting, stopping health monitor".to_string());
        }
    }
        
    let connection_string = conn_info.connection_string
        .ok_or_else(|| "No connection string available".to_string())?;
    
    // Step 3: Test connection and measure response time
    let start_time = ctx.current_time();
    let health_result = ctx
        .schedule_activity_typed::<TestConnectionInput, TestConnectionOutput>(
            activities::TEST_CONNECTION,
            &TestConnectionInput {
                connection_string: connection_string.clone(),
            },
        )
        .into_activity_typed::<TestConnectionOutput>()
        .await;
    
    let response_time_ms = (ctx.current_time() - start_time).as_millis() as i32;
    
    // Step 4: Determine health status and record result
    let (status, postgres_version, error_message) = match health_result {
        Ok(output) => {
            ctx.trace_info(format!("Health check passed ({}ms)", response_time_ms));
            ("healthy", Some(output.version), None)
        }
        Err(e) => {
            ctx.trace_warn(format!("Health check failed: {}", e));
            ("unhealthy", None, Some(e.to_string()))
        }
    };
    
    // Step 5: Record health check in database
    let _record = ctx
            .schedule_activity_typed::<RecordHealthCheckInput, RecordHealthCheckOutput>(
                activities::cms::RECORD_HEALTH_CHECK,
                &RecordHealthCheckInput {
                    k8s_name: input.k8s_name.clone(),
                    status: status.to_string(),
                    postgres_version,
                    response_time_ms: Some(response_time_ms),
                    error_message,
                },
            )
            .into_activity_typed::<RecordHealthCheckOutput>()
            .await
            .map_err(|e| format!("Failed to record health check: {}", e))?;
        
    // Step 6: Update instance health status
    let _update = ctx
        .schedule_activity_typed::<UpdateInstanceHealthInput, UpdateInstanceHealthOutput>(
            activities::cms::UPDATE_INSTANCE_HEALTH,
            &UpdateInstanceHealthInput {
                k8s_name: input.k8s_name.clone(),
                health_status: status.to_string(),
            },
        )
        .into_activity_typed::<UpdateInstanceHealthOutput>()
        .await
        .map_err(|e| format!("Failed to update instance health: {}", e))?;
    
    ctx.trace_info(format!("Health check complete, status: {}", status));
    
    // Step 7: Wait 30 seconds before next check
    ctx.create_timer(std::time::Duration::from_secs(30)).await?;
    
    ctx.trace_info("Restarting health monitor with continue-as-new");
    
    // Step 8: Continue as new to prevent unbounded history growth
    // This ends the current instance and starts a fresh one with the same input
    ctx.continue_as_new(input).await?;
    
    // Note: Code after continue_as_new never executes
    unreachable!("continue_as_new should not return")
}
```

---

## 4. Integration with Create Instance

**File**: `toygres-orchestrations/src/orchestrations/create_instance.rs`

After successful deployment and testing (after Step 4 in current flow):

```rust
// Step 5: Start health monitor orchestration (detached)
ctx.trace_info("Step 5: Starting health monitor");

let health_monitor_id = format!("health-{}", input.name);

let health_input = HealthMonitorInput {
    k8s_name: input.name.clone(),
    namespace: namespace.clone(),
    orchestration_id: health_monitor_id.clone(),
};

// Start as a detached orchestration (runs independently, not a child)
ctx.start_orchestration_detached(
    &health_monitor_id,
    orchestrations::HEALTH_MONITOR,
    serde_json::to_string(&health_input)
        .map_err(|e| format!("Failed to serialize health input: {}", e))?,
)
.await
.map_err(|e| format!("Failed to start health monitor: {}", e))?;

ctx.trace_info(format!("Health monitor started: {}", health_monitor_id));

// Record health monitor orchestration ID in CMS
let _record = ctx
    .schedule_activity_typed::<RecordHealthMonitorInput, RecordHealthMonitorOutput>(
        activities::cms::RECORD_HEALTH_MONITOR,
        &RecordHealthMonitorInput {
            k8s_name: input.name.clone(),
            health_check_orchestration_id: health_monitor_id.clone(),
        },
    )
    .into_activity_typed::<RecordHealthMonitorOutput>()
    .await?;
```

---

## 5. Integration with Delete Instance

**File**: `toygres-orchestrations/src/orchestrations/delete_instance.rs`

After querying CMS record (after Step 0):

```rust
// Step 0.5: Cancel health monitor if it exists
if let Some(health_orch_id) = &cms_record.health_check_orchestration_id {
    ctx.trace_info(format!("Cancelling health monitor: {}", health_orch_id));
    
    // Cancel the health monitor orchestration
    ctx.cancel_orchestration(health_orch_id)
        .await
        .map_err(|e| format!("Failed to cancel health monitor: {}", e))?;
    
    ctx.trace_info("Health monitor cancelled successfully");
}
```

---

## 6. Additional CMS Activity

#### Activity: `record_health_monitor`

**Purpose**: Store health monitor orchestration ID in instances table

**Input**:
```rust
pub struct RecordHealthMonitorInput {
    pub k8s_name: String,
    pub health_check_orchestration_id: String,
}
```

**Output**:
```rust
pub struct RecordHealthMonitorOutput {
    pub recorded: bool,
}
```

**Database Operation**:
```sql
UPDATE toygres_cms.instances
SET health_check_orchestration_id = $2, updated_at = NOW()
WHERE k8s_name = $1
RETURNING id;
```

---

## 7. Registry Updates

**File**: `toygres-orchestrations/src/registry.rs`

```rust
// Add to orchestration registry
registry.register(
    orchestrations::HEALTH_MONITOR,
    "1.0.0",
    health_monitor_orchestration,
);

// Add to activity registry
activities.register(
    activities::cms::RECORD_HEALTH_CHECK,
    record_health_check_activity,
);

activities.register(
    activities::cms::UPDATE_INSTANCE_HEALTH,
    update_instance_health_activity,
);

activities.register(
    activities::cms::GET_INSTANCE_CONNECTION,
    get_instance_connection_activity,
);

activities.register(
    activities::cms::RECORD_HEALTH_MONITOR,
    record_health_monitor_activity,
);
```

---

## 8. Name Constants

**File**: `toygres-orchestrations/src/activity_names.rs`

```rust
pub mod orchestrations {
    pub const HEALTH_MONITOR: &str = "toygres-orchestrations::orchestration::health-monitor";
}

pub mod activities {
    pub mod cms {
        pub const RECORD_HEALTH_CHECK: &str = "toygres-orchestrations::activity::cms-record-health-check";
        pub const UPDATE_INSTANCE_HEALTH: &str = "toygres-orchestrations::activity::cms-update-instance-health";
        pub const GET_INSTANCE_CONNECTION: &str = "toygres-orchestrations::activity::cms-get-instance-connection";
        pub const RECORD_HEALTH_MONITOR: &str = "toygres-orchestrations::activity::cms-record-health-monitor";
    }
}
```

---

## 9. Database Schema

The `instance_health_checks` table already exists (created by `0001_initial_schema.sql`):

```sql
CREATE TABLE toygres_cms.instance_health_checks (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL,
    postgres_version TEXT,
    response_time_ms INTEGER,
    error_message TEXT,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_health_check_time UNIQUE (instance_id, checked_at)
);
```

The `instances` table already has `health_check_orchestration_id`:
```sql
health_check_orchestration_id TEXT
```

✅ **No database migrations needed!**

---

## Implementation Order

### Phase 1: Create CMS Activities
1. ✅ `get_instance_connection` - Retrieve connection string
2. ✅ `record_health_check` - Insert health check result
3. ✅ `update_instance_health` - Update health_status field
4. ✅ `record_health_monitor` - Store orchestration ID

### Phase 2: Create Health Monitor Orchestration
1. ✅ Create `orchestrations/health_monitor.rs`
2. ✅ Implement continue-as-new pattern with 30s timer
3. ✅ Add all activity calls
4. ✅ Add error handling for deleted instances (return Err to exit)
5. ✅ Call `ctx.continue_as_new(input)` to restart with fresh history

### Phase 3: Integrate with Create Instance
1. ✅ Start health monitor as detached orchestration
2. ✅ Record orchestration ID in CMS
3. ✅ Update types to include health_monitor_id in output

### Phase 4: Integrate with Delete Instance
1. ✅ Retrieve health monitor ID from CMS
2. ✅ Cancel the orchestration
3. ✅ Update GetInstanceByK8sNameOutput to include health_check_orchestration_id

### Phase 5: Testing
1. ✅ Create instance and verify health checks start
2. ✅ Check database for health check records
3. ✅ Delete instance and verify health checks stop
4. ✅ Test failure scenarios (instance becomes unhealthy)

---

## Error Handling

### Graceful Exit Scenarios

1. **Instance Deleted**: Health monitor detects state="deleted" and exits gracefully
2. **Instance Not Found**: CMS query returns `found=false`, exit gracefully
3. **Connection Failure**: Record as "unhealthy", continue monitoring

### Non-Graceful Scenarios

1. **CMS Database Unavailable**: Activity fails, Duroxide retries
2. **Orchestration Cancelled**: Duroxide handles cleanup

---

## Testing Strategy

### Unit Tests

1. Test each CMS activity independently
2. Mock database responses
3. Verify SQL queries are correct

### Integration Tests

1. Start health monitor with in-memory SQLite
2. Verify loop iterations
3. Test timer functionality
4. Test graceful exit conditions

### End-to-End Tests

1. Deploy real PostgreSQL instance
2. Verify health checks appear in database every 30s
3. Stop PostgreSQL pod, verify "unhealthy" status
4. Delete instance, verify health checks stop

---

## Observability

### Logs

```
Health check #1 for adardb5-872e2f04
Health check passed (142ms)
Health check complete, status: healthy
```

### Metrics (Future)

- `health_checks_total` - Counter
- `health_check_duration_ms` - Histogram
- `instances_healthy` - Gauge
- `instances_unhealthy` - Gauge

---

## Future Enhancements

1. **Configurable Check Interval**: Pass interval in HealthMonitorInput
2. **Retry Logic**: Retry failed health checks before marking unhealthy
3. **Alert Integration**: Send alerts when instance becomes unhealthy
4. **Health Check History Limit**: Prune old health check records
5. **Custom Health Queries**: Allow custom SQL queries for health validation

---

## Success Criteria

- ✅ Health monitor starts automatically on instance creation
- ✅ Health checks run every 30 seconds
- ✅ Health results stored in `instance_health_checks` table
- ✅ Instance `health_status` field updated correctly
- ✅ Health monitor stops automatically on instance deletion
- ✅ No orphaned health monitors after deletion
- ✅ Graceful handling of instance failures

---

## Questions for Review

1. **Timer Duration**: Is 30 seconds appropriate, or should it be configurable?
2. **Retry Logic**: Should we retry failed health checks before marking unhealthy?
3. **History Retention**: How long should we keep health check records?
4. **Failure Threshold**: Should multiple consecutive failures be required before marking unhealthy?
5. **Cancellation**: Should we use `cancel_orchestration` or let it exit gracefully by detecting deleted state?

