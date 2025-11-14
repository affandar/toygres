# Toygres CMS Database Plan

## Overview

The CMS (Control Management System) database provides persistent storage for tracking provisioned PostgreSQL instances, their configurations, and current state. This complements Duroxide's orchestration state by maintaining business-level metadata.

---

## Database Schema: `toygres_cms`

### Why a Separate Schema?

- **`toygres_duroxide`** - Duroxide framework state (orchestration history, timers, queues)
- **`toygres_cms`** - Application business logic (instance metadata, user mappings, configurations)
- **`public`** - Shared/future extensions

Benefits:
- ✅ Clear separation of concerns
- ✅ Independent backup/restore
- ✅ Different access permissions possible
- ✅ Easy to query business data without Duroxide internals

---

## Table Design

### 1. `instances` Table

Tracks all PostgreSQL instances with their current state.

```sql
CREATE TABLE toygres_cms.instances (
    -- Identity
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_name VARCHAR(255) NOT NULL,              -- User-friendly name (e.g., "mydb")
    k8s_name VARCHAR(255) UNIQUE NOT NULL,         -- K8s resource name (e.g., "mydb-a1b2c3d4")
    
    -- Kubernetes Configuration
    namespace VARCHAR(255) NOT NULL DEFAULT 'toygres',
    
    -- PostgreSQL Configuration
    postgres_version VARCHAR(50) NOT NULL,
    storage_size_gb INTEGER NOT NULL,
    
    -- Networking
    use_load_balancer BOOLEAN NOT NULL,
    dns_name VARCHAR(255),                         -- Full Azure DNS name (UNIQUE when active)
    
    -- Connection Information
    ip_connection_string TEXT,
    dns_connection_string TEXT,
    external_ip VARCHAR(45),                       -- IPv4 or IPv6
    
    -- State Tracking
    state VARCHAR(50) NOT NULL,                    -- creating, running, deleting, deleted, failed
    health_status VARCHAR(50) DEFAULT 'unknown',   -- healthy, unhealthy, unknown
    
    -- Orchestration Tracking
    create_orchestration_id TEXT,                  -- Duroxide orchestration ID for creation
    delete_orchestration_id TEXT,                  -- Duroxide orchestration ID for deletion
    health_check_orchestration_id TEXT,            -- Detached health check orchestration
    
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    
    -- Metadata
    tags JSONB DEFAULT '{}',                       -- Flexible key-value tags
    
    CONSTRAINT valid_state CHECK (state IN ('creating', 'running', 'deleting', 'deleted', 'failed'))
);

-- Indexes for common queries
CREATE INDEX idx_instances_user_name ON toygres_cms.instances(user_name);
CREATE INDEX idx_instances_k8s_name ON toygres_cms.instances(k8s_name);
CREATE INDEX idx_instances_state ON toygres_cms.instances(state);
CREATE INDEX idx_instances_health_status ON toygres_cms.instances(health_status) WHERE state = 'running';
CREATE INDEX idx_instances_namespace ON toygres_cms.instances(namespace);
CREATE INDEX idx_instances_tags ON toygres_cms.instances USING gin(tags);

-- CRITICAL: Unique constraint on dns_name for active instances
-- When deleted/failed, dns_name is prefixed with "__deleted_" to free it up
CREATE UNIQUE INDEX idx_instances_dns_name_unique 
    ON toygres_cms.instances(dns_name) 
    WHERE dns_name IS NOT NULL 
      AND dns_name NOT LIKE '__deleted_%'
      AND state IN ('creating', 'running');

-- Updated timestamp trigger
CREATE OR REPLACE FUNCTION toygres_cms.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_instances_updated_at
    BEFORE UPDATE ON toygres_cms.instances
    FOR EACH ROW
    EXECUTE FUNCTION toygres_cms.update_updated_at_column();
```

### 2. `instance_events` Table

Audit log of state transitions and important events.

```sql
CREATE TABLE toygres_cms.instance_events (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES toygres_cms.instances(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,              -- state_change, health_check, error, dns_conflict, etc.
    old_state VARCHAR(50),
    new_state VARCHAR(50),
    message TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_instance_events_instance_id ON toygres_cms.instance_events(instance_id);
CREATE INDEX idx_instance_events_type ON toygres_cms.instance_events(event_type);
CREATE INDEX idx_instance_events_created_at ON toygres_cms.instance_events(created_at DESC);
```

### 4. `drift_detections` Table

Records found by garbage collector when CMS and K8s are out of sync.

```sql
CREATE TABLE toygres_cms.drift_detections (
    id BIGSERIAL PRIMARY KEY,
    detection_type VARCHAR(100) NOT NULL,          -- orphaned_k8s, orphaned_cms, state_mismatch
    k8s_name VARCHAR(255),
    cms_state VARCHAR(50),
    k8s_exists BOOLEAN,
    message TEXT,
    metadata JSONB DEFAULT '{}',
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,                       -- When drift was resolved
    resolved_by VARCHAR(100)                       -- How it was resolved (auto_cleaned, manual, etc.)
);

CREATE INDEX idx_drift_detections_type ON toygres_cms.drift_detections(detection_type);
CREATE INDEX idx_drift_detections_detected_at ON toygres_cms.drift_detections(detected_at DESC);
CREATE INDEX idx_drift_detections_unresolved ON toygres_cms.drift_detections(resolved_at) WHERE resolved_at IS NULL;
```

### 3. `instance_health_checks` Table

Historical health check results for monitoring and alerting.

```sql
CREATE TABLE toygres_cms.instance_health_checks (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES toygres_cms.instances(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL,                   -- healthy, unhealthy
    postgres_version TEXT,
    response_time_ms INTEGER,
    error_message TEXT,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure idempotency: can't insert duplicate health check for same instance at same time
    CONSTRAINT unique_health_check_time UNIQUE (instance_id, checked_at)
);

CREATE INDEX idx_health_checks_instance_id ON toygres_cms.instance_health_checks(instance_id);
CREATE INDEX idx_health_checks_checked_at ON toygres_cms.instance_health_checks(checked_at DESC);

-- Partition by month for scalability (optional, for large deployments)
-- CREATE TABLE toygres_cms.instance_health_checks_2025_11 PARTITION OF toygres_cms.instance_health_checks
--     FOR VALUES FROM ('2025-11-01') TO ('2025-12-01');
```

---

## Integration with Duroxide Orchestrations

### Pattern: Activity for CMS Operations

Create new activities in `toygres-orchestrations/src/activities/cms/`:

```rust
// activities/cms/create_instance_record.rs
pub async fn create_instance_record_activity(
    ctx: ActivityContext,
    input: CreateInstanceRecordInput,
) -> Result<CreateInstanceRecordOutput, String> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&db_url).await?;
    let mut tx = pool.begin().await?;
    
    let insert_result = sqlx::query!(
        r#"
        INSERT INTO toygres_cms.instances 
        (user_name, k8s_name, namespace, postgres_version, storage_size_gb, 
         use_load_balancer, dns_name, state, create_orchestration_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'creating', $8)
        ON CONFLICT (k8s_name) DO UPDATE
        SET user_name = EXCLUDED.user_name,
            namespace = EXCLUDED.namespace,
            postgres_version = EXCLUDED.postgres_version,
            storage_size_gb = EXCLUDED.storage_size_gb,
            use_load_balancer = EXCLUDED.use_load_balancer,
            dns_name = EXCLUDED.dns_name,
            updated_at = NOW()
        WHERE toygres_cms.instances.create_orchestration_id = EXCLUDED.create_orchestration_id
        RETURNING id
        "#,
        input.user_name,
        input.k8s_name,
        input.namespace,
        input.postgres_version,
        input.storage_size_gb,
        input.use_load_balancer,
        input.dns_name,
        input.orchestration_id
    )
    .fetch_optional(&mut *tx)
    .await;
    
    match insert_result {
        Ok(Some(row)) => {
            tx.commit().await.map_err(|e| format!("tx commit failed: {}", e))?;
            ctx.trace_info(format!("Created/found CMS record for instance: {}", input.k8s_name));
            Ok(CreateInstanceRecordOutput { instance_id: row.id })
        }
        Err(sqlx::Error::Database(db_err))
            if db_err.code().as_deref() == Some("23505")
               && db_err.constraint() == Some("idx_instances_dns_name_unique") =>
        {
            // DNS name is already present. Grab the existing record inside the same transaction.
            let dns_name = input
                .dns_name
                .clone()
                .ok_or_else(|| "DNS name missing in conflict path".to_string())?;
            
            let existing = sqlx::query!(
                r#"
                SELECT id, k8s_name, user_name, create_orchestration_id
                FROM toygres_cms.instances
                WHERE dns_name = $1
                  AND dns_name NOT LIKE '__deleted_%'
                  AND state IN ('creating', 'running')
                FOR UPDATE
                "#,
                dns_name
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to load DNS owner: {}", e))?;
            
            if let Some(conflict) = existing {
                if conflict.create_orchestration_id == input.orchestration_id {
                    // Same orchestration replaying with the same DNS; treat as idempotent.
                    tx.commit().await.map_err(|e| format!("tx commit failed: {}", e))?;
                    ctx.trace_info(format!(
                        "Reusing CMS record {} (DNS replay, k8s_name: {})",
                        conflict.id, conflict.k8s_name
                    ));
                    Ok(CreateInstanceRecordOutput {
                        instance_id: conflict.id,
                    })
                } else {
                    tx.rollback()
                        .await
                        .map_err(|e| format!("tx rollback failed: {}", e))?;
                    Err(format!(
                        "DNS name '{}' is already in use by instance '{}' (orch: {})",
                        dns_name, conflict.k8s_name, conflict.create_orchestration_id
                    ))
                }
            } else {
                tx.rollback()
                    .await
                    .map_err(|e| format!("tx rollback failed: {}", e))?;
                Err("DNS name constraint hit but record vanished; retry".into())
            }
        }
        Err(e) => {
            tx.rollback()
                .await
                .map_err(|err| format!("tx rollback failed after error: {}", err))?;
            Err(format!("Failed to create instance record: {}", e))
        }
        Ok(None) => unreachable!("RETURNING always yields a row on success"),
    }
}

// activities/cms/update_instance_state.rs
pub async fn update_instance_state_activity(
    ctx: ActivityContext,
    input: UpdateInstanceStateInput,
) -> Result<UpdateInstanceStateOutput, String> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&db_url).await?;
    
    // IDEMPOTENT: Check current state before updating
    let current = sqlx::query!(
        r#"SELECT state FROM toygres_cms.instances WHERE k8s_name = $1"#,
        input.k8s_name
    )
    .fetch_optional(&pool)
    .await?;
    
    let current_state = current.map(|r| r.state);
    
    // Only update if state is different (idempotency)
    if current_state.as_deref() != Some(&input.state) {
        sqlx::query!(
            r#"
            UPDATE toygres_cms.instances 
            SET state = $1, 
                ip_connection_string = COALESCE($2, ip_connection_string),
                dns_connection_string = COALESCE($3, dns_connection_string),
                external_ip = COALESCE($4, external_ip),
                dns_name = COALESCE($5, dns_name),
                updated_at = NOW()
            WHERE k8s_name = $6
            "#,
            input.state,
            input.ip_connection_string,
            input.dns_connection_string,
            input.external_ip,
            input.dns_name,
            input.k8s_name
        )
        .execute(&pool)
        .await?;
        
        ctx.trace_info(format!("Updated instance state: {} → {}", 
                               current_state.as_deref().unwrap_or("unknown"), 
                               input.state));
        
        // Log state transition event (IDEMPOTENT: only if state changed)
        if let Some(old_state) = current_state {
            sqlx::query!(
                r#"
                INSERT INTO toygres_cms.instance_events 
                (instance_id, event_type, old_state, new_state, message)
                SELECT id, 'state_change', $1, $2, $3
                FROM toygres_cms.instances
                WHERE k8s_name = $4
                "#,
                old_state,
                input.state,
                input.message,
                input.k8s_name
            )
            .execute(&pool)
            .await?;
        }
    } else {
        ctx.trace_info(format!("Instance already in state '{}', skipping update", input.state));
    }
    
    Ok(UpdateInstanceStateOutput { success: true })
}

// activities/cms/record_health_check.rs
pub async fn record_health_check_activity(
    ctx: ActivityContext,
    input: RecordHealthCheckInput,
) -> Result<RecordHealthCheckOutput, String> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&db_url).await?;
    
    // IDEMPOTENT: Use check_time or orchestration context time to avoid duplicates
    // Include orchestration execution context to make insertion deterministic
    let check_time = input.check_time; // From orchestration ctx.utcnow_ms()
    
    // Insert health check result (idempotent based on timestamp)
    let inserted = sqlx::query!(
        r#"
        INSERT INTO toygres_cms.instance_health_checks 
        (instance_id, status, postgres_version, response_time_ms, error_message, checked_at)
        SELECT id, $1, $2, $3, $4, $5
        FROM toygres_cms.instances
        WHERE k8s_name = $6
        ON CONFLICT DO NOTHING
        RETURNING id
        "#,
        input.status,
        input.postgres_version,
        input.response_time_ms,
        input.error_message,
        check_time,
        input.k8s_name
    )
    .fetch_optional(&pool)
    .await?;
    
    if inserted.is_some() {
        ctx.trace_info("Health check result recorded");
        
        // Update instance health status (IDEMPOTENT: always safe to update)
        sqlx::query!(
            r#"
            UPDATE toygres_cms.instances 
            SET health_status = $1
            WHERE k8s_name = $2
            "#,
            input.status,
            input.k8s_name
        )
        .execute(&pool)
        .await?;
    } else {
        ctx.trace_info("Health check result already recorded (replay), skipping");
    }
    
    Ok(RecordHealthCheckOutput { success: true })
}

// activities/cms/get_instance_by_user_name.rs
pub async fn get_instance_by_user_name_activity(
    ctx: ActivityContext,
    input: GetInstanceByUserNameInput,
) -> Result<GetInstanceByUserNameOutput, String> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&db_url).await?;
    
    // Query instance by user name
    let instance = sqlx::query_as!(
        InstanceRecord,
        r#"
        SELECT id, user_name, k8s_name, namespace, state, 
               ip_connection_string, dns_connection_string
        FROM toygres_cms.instances
        WHERE user_name = $1 AND state != 'deleted'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        input.user_name
    )
    .fetch_optional(&pool)
    .await?;
    
    Ok(GetInstanceByUserNameOutput { instance })
}

// activities/cms/free_dns_name.rs
pub async fn free_dns_name_activity(
    ctx: ActivityContext,
    input: FreeDnsNameInput,
) -> Result<FreeDnsNameOutput, String> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&db_url).await?;
    
    // IDEMPOTENT: Prefix dns_name with __deleted_ to free it for reuse
    // If already prefixed, this is a no-op
    let updated = sqlx::query!(
        r#"
        UPDATE toygres_cms.instances
        SET dns_name = CASE 
            WHEN dns_name IS NOT NULL AND dns_name NOT LIKE '__deleted_%' 
            THEN '__deleted_' || dns_name
            ELSE dns_name
        END
        WHERE k8s_name = $1
          AND dns_name IS NOT NULL
          AND dns_name NOT LIKE '__deleted_%'
        "#,
        input.k8s_name
    )
    .execute(&pool)
    .await?
    .rows_affected();
    
    if updated > 0 {
        ctx.trace_info(format!("Freed DNS name for instance: {}", input.k8s_name));
    } else {
        ctx.trace_info("DNS name already freed or not set (replay)");
    }
    
    Ok(FreeDnsNameOutput { freed: updated > 0 })
}

// activities/cms/list_k8s_deployments.rs
pub async fn list_k8s_deployments_activity(
    ctx: ActivityContext,
    input: ListK8sDeploymentsInput,
) -> Result<ListK8sDeploymentsOutput, String> {
    let client = get_k8s_client().await?;
    let statefulsets: Api<StatefulSet> = Api::namespaced(client, &input.namespace);
    
    // List all PostgreSQL StatefulSets
    let sts_list = statefulsets
        .list(&ListParams::default().labels("app=postgres"))
        .await?;
    
    let deployments: Vec<K8sDeployment> = sts_list.items.iter()
        .filter_map(|sts| {
            sts.metadata.name.as_ref().map(|name| K8sDeployment {
                k8s_name: name.clone(),
                replicas: sts.spec.as_ref()?.replicas?,
                ready_replicas: sts.status.as_ref()?.ready_replicas?,
            })
        })
        .collect();
    
    ctx.trace_info(format!("Found {} K8s deployments", deployments.len()));
    
    Ok(ListK8sDeploymentsOutput { deployments })
}

// activities/cms/record_drift.rs
pub async fn record_drift_activity(
    ctx: ActivityContext,
    input: RecordDriftInput,
) -> Result<RecordDriftOutput, String> {
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::PgPool::connect(&db_url).await?;
    
    // IDEMPOTENT: Use unique constraint or check before inserting
    // For now, always insert (garbage collector runs periodically)
    sqlx::query!(
        r#"
        INSERT INTO toygres_cms.drift_detections
        (detection_type, k8s_name, cms_state, k8s_exists, message, metadata)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        input.detection_type,
        input.k8s_name,
        input.cms_state,
        input.k8s_exists,
        input.message,
        input.metadata
    )
    .execute(&pool)
    .await?;
    
    ctx.trace_warn(format!("Recorded drift: {} for instance {}", 
                           input.detection_type, 
                           input.k8s_name.as_deref().unwrap_or("unknown")));
    
    Ok(RecordDriftOutput { recorded: true })
}
```

---

## Updated Orchestration Flows

### CreateInstanceOrchestration (Updated)

```
┌─────────────────────────────────────────────────┐
│ CreateInstanceOrchestration                     │
├─────────────────────────────────────────────────┤
│ Input: user_name, password, config              │
│                                                  │
│ 1. Generate k8s_name (user_name + GUID)         │
│                                                  │
│ 2. CREATE_INSTANCE_RECORD (CMS activity)        │
│    → INSERT with ON CONFLICT logic:             │
│      - If dns_name conflict + same orch_id      │
│        → UPDATE (idempotent replay)             │
│      - If dns_name conflict + diff orch_id      │
│        → ERROR (DNS locked by another orch)     │
│    → State: 'creating'                           │
│    → Store orchestration_id and dns_name        │
│                                                  │
│ 4. DEPLOY_POSTGRES (K8s activity)               │
│    → Create K8s resources                        │
│                                                  │
│ 5. WAIT_FOR_READY (poll loop)                   │
│    → Check pod status                            │
│                                                  │
│ 6. GET_CONNECTION_STRINGS (K8s activity)        │
│    → Get LoadBalancer IP & DNS                   │
│                                                  │
│ 7. UPDATE_INSTANCE_STATE (CMS activity)         │
│    → State: 'running'                            │
│    → Save connection strings                     │
│    → Log state transition event                  │
│                                                  │
│ 8. TEST_CONNECTION (K8s activity)               │
│    → Verify PostgreSQL responds                  │
│                                                  │
│ 9. RECORD_HEALTH_CHECK (CMS activity)           │
│    → Save initial health check result            │
│                                                  │
│ 10. START_HEALTH_CHECK_ORCHESTRATION (detached) │
│    → Start continuous monitoring                 │
│    → Store health_check_orchestration_id         │
│                                                  │
│ Output: Instance ID, connection strings          │
│                                                  │
│ On Failure:                                      │
│   → Call DeleteInstanceOrchestration            │
│   → Updates CMS: dns_name = '__deleted_' prefix │
│   → Updates CMS state to 'failed' or 'deleted'  │
│   → Frees DNS name for reuse                    │
└─────────────────────────────────────────────────┘
```

### DeleteInstanceOrchestration (Updated)

```
┌─────────────────────────────────────────────────┐
│ DeleteInstanceOrchestration                     │
├─────────────────────────────────────────────────┤
│ Input: user_name (or k8s_name)                  │
│                                                  │
│ 1. GET_INSTANCE_BY_USER_NAME (CMS activity)     │
│    → Lookup k8s_name from user_name             │
│    → Get health_check_orchestration_id          │
│                                                  │
│ 2. UPDATE_INSTANCE_STATE (CMS activity)         │
│    → State: 'deleting'                           │
│    → Store delete_orchestration_id              │
│                                                  │
│ 3. If health_check_orchestration_id exists:     │
│    → Cancel health check orchestration           │
│                                                  │
│ 4. DELETE_POSTGRES (K8s activity)               │
│    → Delete K8s resources                        │
│                                                  │
│ 5. FREE_DNS_NAME (CMS activity)                 │
│    → Prefix dns_name with '__deleted_'          │
│    → Releases DNS name for reuse                │
│                                                  │
│ 6. UPDATE_INSTANCE_STATE (CMS activity)         │
│    → State: 'deleted'                            │
│    → Set deleted_at timestamp                    │
│                                                  │
│ Output: Deletion status                          │
└─────────────────────────────────────────────────┘
```

### HealthCheckOrchestration (New - Phase 3)

```
┌─────────────────────────────────────────────────┐
│ HealthCheckOrchestration (continuous)           │
├─────────────────────────────────────────────────┤
│ Input: k8s_name                                  │
│                                                  │
│ Loop forever:                                    │
│   1. GET_INSTANCE_BY_K8S_NAME (CMS)             │
│      → Get current connection string             │
│      → Check if still in 'running' state         │
│                                                  │
│   2. If state != 'running':                      │
│      → Exit (instance was deleted)               │
│                                                  │
│   3. TEST_CONNECTION (K8s activity)             │
│      → Connect and query PostgreSQL              │
│      → Measure response time                     │
│                                                  │
│   4. RECORD_HEALTH_CHECK (CMS activity)         │
│      → Save health check result                  │
│      → Update instance.health_status             │
│      → Use ctx.utcnow_ms() for timestamp         │
│                                                  │
│   5. Schedule 30-second timer (Duroxide)        │
│      → Wait before next check                    │
│                                                  │
│ Lifecycle: Cancelled by DeleteInstanceOrch      │
└─────────────────────────────────────────────────┘
```

### GarbageCollectorOrchestration (New - Phase 3)

```
┌─────────────────────────────────────────────────┐
│ GarbageCollectorOrchestration (continuous)      │
├─────────────────────────────────────────────────┤
│ Purpose: Detect drift between CMS and K8s       │
│                                                  │
│ Loop forever:                                    │
│   1. LIST_INSTANCES (CMS activity)              │
│      → Get all non-deleted instances            │
│                                                  │
│   2. LIST_K8S_DEPLOYMENTS (K8s activity)        │
│      → Get all StatefulSets with app=postgres   │
│                                                  │
│   3. Compare CMS vs K8s:                         │
│      a) CMS shows 'running' but K8s missing     │
│         → ORPHANED_CMS drift                     │
│         → Instance in CMS with no K8s resources  │
│                                                  │
│      b) K8s exists but not in CMS               │
│         → ORPHANED_K8S drift                     │
│         → Untracked deployment                   │
│                                                  │
│      c) CMS state doesn't match K8s reality     │
│         → STATE_MISMATCH drift                   │
│         → CMS says 'running' but pod Pending    │
│                                                  │
│   4. For each drift found:                       │
│      → RECORD_DRIFT (CMS activity)              │
│      → Log to drift_detections table            │
│      → Increment drift counter                   │
│                                                  │
│   5. If drifts found:                            │
│      → Log warning with count                    │
│      → (Future: auto-remediation)                │
│                                                  │
│   6. Schedule 5-minute timer                     │
│      → Wait before next scan                     │
│                                                  │
│ Lifecycle: Long-running, started once           │
└─────────────────────────────────────────────────┘
```

---

## New CMS Activities

### Activity Names (add to `activity_names.rs`)

```rust
pub mod cms {
    /// Create instance record in CMS (with DNS locking)
    pub const CREATE_INSTANCE_RECORD: &str = "toygres-orchestrations::activity::cms-create-instance-record";
    
    /// Update instance state in CMS
    pub const UPDATE_INSTANCE_STATE: &str = "toygres-orchestrations::activity::cms-update-instance-state";
    
    /// Free DNS name by prefixing with __deleted_
    pub const FREE_DNS_NAME: &str = "toygres-orchestrations::activity::cms-free-dns-name";
    
    /// Record health check result in CMS
    pub const RECORD_HEALTH_CHECK: &str = "toygres-orchestrations::activity::cms-record-health-check";
    
    /// Get instance by user name from CMS
    pub const GET_INSTANCE_BY_USER_NAME: &str = "toygres-orchestrations::activity::cms-get-instance-by-user-name";
    
    /// Get instance by k8s name from CMS
    pub const GET_INSTANCE_BY_K8S_NAME: &str = "toygres-orchestrations::activity::cms-get-instance-by-k8s-name";
    
    /// List all instances from CMS
    pub const LIST_INSTANCES: &str = "toygres-orchestrations::activity::cms-list-instances";
    
    /// List all K8s PostgreSQL deployments
    pub const LIST_K8S_DEPLOYMENTS: &str = "toygres-orchestrations::activity::cms-list-k8s-deployments";
    
    /// Record drift detection
    pub const RECORD_DRIFT: &str = "toygres-orchestrations::activity::cms-record-drift";
}
```

### Activity Types (add to `activity_types.rs`)

```rust
// FreeDnsName
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeDnsNameInput {
    pub k8s_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeDnsNameOutput {
    pub freed: bool,
}

// ListK8sDeployments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListK8sDeploymentsInput {
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListK8sDeploymentsOutput {
    pub deployments: Vec<K8sDeployment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct K8sDeployment {
    pub k8s_name: String,
    pub replicas: i32,
    pub ready_replicas: i32,
}

// RecordDrift
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordDriftInput {
    pub detection_type: String,  // orphaned_cms, orphaned_k8s, state_mismatch
    pub k8s_name: Option<String>,
    pub cms_state: Option<String>,
    pub k8s_exists: bool,
    pub message: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordDriftOutput {
    pub recorded: bool,
}

// CreateInstanceRecord
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceRecordInput {
    pub user_name: String,
    pub k8s_name: String,
    pub namespace: String,
    pub postgres_version: String,
    pub storage_size_gb: i32,
    pub use_load_balancer: bool,
    pub dns_name: Option<String>,                   // Full DNS name (e.g., mydb-toygres.westus3.cloudapp.azure.com)
    pub orchestration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateInstanceRecordOutput {
    pub instance_id: uuid::Uuid,
}

// UpdateInstanceState
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInstanceStateInput {
    pub k8s_name: String,
    pub state: String,
    pub old_state: Option<String>,
    pub ip_connection_string: Option<String>,
    pub dns_connection_string: Option<String>,
    pub external_ip: Option<String>,
    pub dns_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateInstanceStateOutput {
    pub success: bool,
}

// RecordHealthCheck
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordHealthCheckInput {
    pub k8s_name: String,
    pub status: String,
    pub postgres_version: Option<String>,
    pub response_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub check_time: chrono::DateTime<chrono::Utc>,  // From ctx.utcnow_ms() for idempotency
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordHealthCheckOutput {
    pub success: bool,
}

// GetInstanceByUserName
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetInstanceByUserNameInput {
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetInstanceByUserNameOutput {
    pub instance: Option<InstanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceRecord {
    pub id: uuid::Uuid,
    pub user_name: String,
    pub k8s_name: String,
    pub namespace: String,
    pub state: String,
    pub ip_connection_string: Option<String>,
    pub dns_connection_string: Option<String>,
    pub health_status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

---

## Database Connection Pattern

### Option 1: Connection Per Activity (Simpler)

```rust
pub async fn my_cms_activity(ctx: ActivityContext, input: Input) -> Result<Output, String> {
    // Get DB URL from environment
    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL not set".to_string())?;
    
    // Create connection pool (cached by sqlx internally)
    let pool = sqlx::PgPool::connect(&db_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
    // Use pool for queries
    // ...
    
    Ok(output)
}
```

**Pros:**
- Simple, each activity is self-contained
- No shared state needed
- sqlx handles connection pooling

**Cons:**
- Slight overhead creating pool each time (mitigated by sqlx caching)

### Option 2: Shared Pool (More Efficient)

Pass database pool through activity context configuration:

```rust
// In main.rs or worker setup
let db_pool = Arc::new(sqlx::PgPool::connect(&db_url).await?);

// Store in activity registry somehow, or use thread-local/lazy_static
lazy_static! {
    static ref DB_POOL: Mutex<Option<Arc<PgPool>>> = Mutex::new(None);
}

// Activities access the shared pool
pub async fn my_cms_activity(ctx: ActivityContext, input: Input) -> Result<Output, String> {
    let pool = get_db_pool()?; // Get from static/thread-local
    // Use pool...
}
```

**Recommendation:** Start with Option 1 (connection per activity). It's simpler and sqlx handles pooling efficiently.

---

## Query Patterns

### User-Friendly Lookups

```sql
-- Get instance by user name (most recent if multiple)
SELECT * FROM toygres_cms.instances 
WHERE user_name = 'mydb' 
  AND state != 'deleted'
ORDER BY created_at DESC 
LIMIT 1;

-- List all running instances for a user
SELECT user_name, k8s_name, ip_connection_string, health_status
FROM toygres_cms.instances
WHERE user_name LIKE 'prod-%'
  AND state = 'running'
ORDER BY created_at DESC;
```

### Operations Tracking

```sql
-- Find instance by Duroxide orchestration ID
SELECT * FROM toygres_cms.instances
WHERE create_orchestration_id = 'create-mydb-a1b2c3d4';

-- Get all events for an instance
SELECT event_type, old_state, new_state, message, created_at
FROM toygres_cms.instance_events
WHERE instance_id = '...'
ORDER BY created_at DESC;
```

### Health Monitoring

```sql
-- Recent health checks
SELECT i.user_name, i.k8s_name, h.status, h.checked_at
FROM toygres_cms.instances i
JOIN toygres_cms.instance_health_checks h ON i.id = h.instance_id
WHERE h.checked_at > NOW() - INTERVAL '1 hour'
ORDER BY h.checked_at DESC;

-- Unhealthy instances
SELECT user_name, k8s_name, health_status, updated_at
FROM toygres_cms.instances
WHERE state = 'running' 
  AND health_status = 'unhealthy';
```

---

## Migration Strategy

### Phase 3.1: Add CMS Activities

1. Create `src/activities/cms/` directory
2. Implement 5 CMS activities
3. Register in `registry.rs`
4. Add activity types to `activity_types.rs`
5. Unit tests for each activity

### Phase 3.2: Update CreateInstanceOrchestration

1. Add `CREATE_INSTANCE_RECORD` at start
2. Add `UPDATE_INSTANCE_STATE` after getting connection strings
3. Add `RECORD_HEALTH_CHECK` after testing connection
4. Update cleanup to mark instance as 'failed' or 'deleted'

### Phase 3.3: Update DeleteInstanceOrchestration

1. Add `GET_INSTANCE_BY_USER_NAME` at start (allows delete by user name)
2. Add state transitions to CMS
3. Cancel health check orchestration if exists
4. Mark as 'deleted' in CMS

### Phase 3.4: Implement HealthCheckOrchestration

1. Create new orchestration
2. Infinite loop with timer
3. Query CMS for instance details
4. Test connection
5. Record health check result
6. Update CMS health status

### Phase 3.5: Update CLI

1. Delete by user name instead of k8s name
2. Add list command (queries CMS)
3. Add status command (shows instance details from CMS)
4. Show health check history

---

## CLI Improvements

### New Commands

```rust
enum Commands {
    Create { /* existing */ },
    Delete { name: String },  // Now accepts user_name, looks up k8s_name
    
    /// List all instances
    List {
        #[arg(long)]
        state: Option<String>,  // Filter by state
    },
    
    /// Get instance details
    Status {
        name: String,  // User name
    },
    
    /// Show health check history
    Health {
        name: String,  // User name
        #[arg(long, default_value = "10")]
        limit: i32,
    },
    
    /// Check for drift between CMS and K8s
    CheckDrift {
        /// Automatically resolve simple drifts
        #[arg(long)]
        auto_resolve: bool,
    },
}
```

### Usage

```bash
# Create (same as before, but returns user-friendly name)
cargo run --bin toygres-server create mydb --password test123

# Delete by user name (no need to remember GUID)
cargo run --bin toygres-server delete mydb

# List all instances
cargo run --bin toygres-server list

# List only running instances
cargo run --bin toygres-server list --state running

# Get instance status
cargo run --bin toygres-server status mydb

# Show health history
cargo run --bin toygres-server health mydb --limit 20

# Check for drift
cargo run --bin toygres-server check-drift

# Check drift with auto-resolve
cargo run --bin toygres-server check-drift --auto-resolve
```

---

## Benefits of CMS Database

### 1. User-Friendly Operations

**Without CMS:**
```bash
cargo run --bin toygres-server delete mydb-a1b2c3d4  # Need to remember GUID
```

**With CMS:**
```bash
cargo run --bin toygres-server delete mydb  # Use friendly name
```

### 2. State Tracking

- Know which instances exist and their states
- Track creation/deletion orchestration IDs
- Audit trail of all state changes

### 3. Health Monitoring

- Historical health data for analysis
- Detect degraded instances
- Alert on unhealthy instances

### 4. Multi-User Support

- Track who created what
- Filter by user/project
- Resource quotas per user

### 5. REST API Foundation

CMS makes REST API simple:

```rust
// GET /instances
async fn list_instances() -> Json<Vec<InstanceRecord>> {
    // Query toygres_cms.instances
}

// GET /instances/{user_name}
async fn get_instance(name: String) -> Json<InstanceRecord> {
    // Query by user_name
}
```

---

## Data Consistency

### Duroxide ↔ CMS Consistency

**Problem:** Orchestration state (Duroxide) vs Instance state (CMS) can diverge

**Solution:** CMS is source of truth for business logic

```
┌──────────────────────────────────────────┐
│ Duroxide (toygres_duroxide schema)       │
│ - Orchestration history                  │
│ - Activity results                       │
│ - Timers, queues                         │
│ - Technical execution details            │
└──────────────────────────────────────────┘
                 ↓
         Updates via activities
                 ↓
┌──────────────────────────────────────────┐
│ CMS (toygres_cms schema)                 │
│ - Instance metadata                      │
│ - Current state                          │
│ - Connection strings                     │
│ - Business-level information             │
└──────────────────────────────────────────┘
```

**Consistency Rules:**
1. Every state change MUST update CMS (via activity)
2. **CMS activities are idempotent** (critical for Duroxide replays)
3. On orchestration replay, CMS updates are no-ops or idempotent updates
4. CMS queries are read-only from orchestrations (determinism)

**Idempotency Strategies:**

1. **CREATE operations** - Use `INSERT ... ON CONFLICT DO UPDATE` (UPSERT)
   ```sql
   INSERT INTO instances (...) VALUES (...)
   ON CONFLICT (k8s_name) DO UPDATE SET ...
   RETURNING id
   ```

2. **UPDATE operations** - Check current value before updating
   ```sql
   -- Only update if different
   UPDATE instances SET state = $1 
   WHERE k8s_name = $2 AND state != $1
   ```

3. **INSERT events** - Use unique constraints with deterministic timestamps
   ```sql
   -- Use orchestration context time, not NOW()
   INSERT INTO instance_health_checks (instance_id, checked_at, ...)
   VALUES (..., $orchestration_time, ...)
   ON CONFLICT (instance_id, checked_at) DO NOTHING
   ```

4. **State transitions** - Conditional updates with old state check
   ```sql
   -- Only transition from expected state
   UPDATE instances SET state = 'running'
   WHERE k8s_name = $1 AND state = 'creating'
   ```

---

## Schema Migration Scripts

Follow the same pattern as `duroxide-pg`:

- Place migrations under `migrations/cms/` using numbered files
  (`0001_initial_schema.sql`, `0002_add_column.sql`, …).
- Track applied migrations in `toygres_cms._toygres_migrations`.
- Provide scripts for initial provisioning and incremental migrations.

### `scripts/db-init.sh`

- Loads `.env`, ensures `DATABASE_URL` is present.
- Creates the `toygres_cms` schema plus the `_toygres_migrations` tracking table.
- Applies `migrations/cms/0001_initial_schema.sql` exactly once.
- Delegates to `scripts/db-migrate.sh` (so new migrations run automatically on fresh installs).
- `scripts/setup-db.sh` remains as a thin wrapper for backwards compatibility.

### `scripts/db-migrate.sh`

- Loads `.env` and ensures the tracking table exists.
- Applies any migration numbered `0002` or greater that has not been recorded yet.
- Logs when there are no pending migrations (current state).

---

## Implementation Order

### Phase 3.1: Database Schema
1. Add `migrations/cms/0001_initial_schema.sql`
2. Implement `scripts/db-init.sh` (plus `scripts/db-migrate.sh`)
3. Test schema creation via `./scripts/db-init.sh`

### Phase 3.2: CMS Activities
1. Implement 5 CMS activities
2. Add types and names
3. Register in registry
4. Unit tests

### Phase 3.3: Update Orchestrations
1. Add CMS calls to CreateInstanceOrchestration
2. Add CMS calls to DeleteInstanceOrchestration
3. Test end-to-end

### Phase 3.4: Health Check Orchestration
1. Implement continuous health monitoring
2. Integrate with CreateInstance (start detached)
3. Integrate with DeleteInstance (cancel)

### Phase 3.5: Garbage Collector Orchestration
1. Implement drift detection logic
2. Compare CMS instances vs K8s StatefulSets
3. Record drifts to database
4. Add auto-resolve option (Phase 4)

### Phase 3.6: Update CLI
1. Delete by user name
2. Add list/status/health commands
3. Add check-drift command
4. User-friendly output

---

## Database Pool Management

### Recommended Approach

Create a shared database module in `toygres-orchestrations`:

```rust
// src/db.rs
use sqlx::PgPool;
use std::sync::OnceLock;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

pub async fn init_db_pool(database_url: &str) -> anyhow::Result<()> {
    let pool = PgPool::connect(database_url).await?;
    DB_POOL.set(pool).map_err(|_| anyhow::anyhow!("DB pool already initialized"))?;
    Ok(())
}

pub fn get_db_pool() -> Result<&'static PgPool, String> {
    DB_POOL.get().ok_or_else(|| "Database pool not initialized".to_string())
}

// In activities
pub async fn my_activity(ctx: ActivityContext, input: Input) -> Result<Output, String> {
    let pool = crate::db::get_db_pool()?;
    // Use pool...
}
```

Initialize in `main.rs`:

```rust
// Before starting runtime
toygres_orchestrations::db::init_db_pool(&db_url).await?;
```

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_create_instance_record() {
    let pool = create_test_pool().await;
    
    // Test CMS activity
    let input = CreateInstanceRecordInput { /* ... */ };
    let output = create_instance_record_activity(ctx, input).await.unwrap();
    
    // Verify in database
    let row = sqlx::query!("SELECT * FROM toygres_cms.instances WHERE id = $1", output.instance_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(row.state, "creating");
}
```

### Integration Tests

Test orchestrations with real database:

```rust
#[tokio::test]
async fn test_create_instance_with_cms() {
    let test_db = setup_test_database().await;  // Temporary test DB
    let runtime = setup_duroxide_runtime(test_db.url()).await;
    
    // Start orchestration
    // Verify CMS records
    // Verify K8s resources
    // Verify Duroxide history
}
```

---

## Questions Before Implementation

1. **Tags/Labels**: Should instances support arbitrary tags/labels for organization?
2. **Multi-tenancy**: Should we track owner/tenant information?
3. **Quotas**: Should CMS enforce per-user resource limits?
4. **Audit Trail**: Keep deleted instances in DB or hard delete?
5. **Health Check Retention**: How long to keep health check history? (30 days? 90 days?)
6. **Connection Pool**: Shared pool or per-activity connections?

---

## Success Criteria for Phase 3

✅ CMS schema created with 3 tables  
✅ 5 CMS activities implemented  
✅ CreateInstance tracks state in CMS  
✅ DeleteInstance uses user name lookup  
✅ Health check orchestration records to CMS  
✅ CLI supports user-friendly operations  
✅ All orchestrations maintain CMS consistency  
✅ Tests verify CMS and K8s state match  

---

## Next Steps

1. Review this plan and answer questions
2. Finalize migration scripts (`db-init.sh`, `db-migrate.sh`)
3. Implement CMS activities
4. Update orchestrations to use CMS
5. Add health check orchestration
6. Update CLI for better UX

