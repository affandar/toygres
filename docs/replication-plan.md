# PostgreSQL Replication Plan - Synchronous Standby for Toygres

## Overview

Add automatic synchronous replication to every PostgreSQL instance deployed by Toygres. Each primary instance will have a standby replica with synchronous replication enabled for high availability and zero data loss.

---

## PostgreSQL Streaming Replication Fundamentals

### How PostgreSQL Replication Works

**Streaming Replication** is PostgreSQL's built-in replication mechanism:

1. **Primary Server** writes changes to Write-Ahead Log (WAL)
2. **WAL Sender** process streams WAL records to standbys
3. **WAL Receiver** process on standby receives and applies changes
4. **Standby** continuously replays WAL to stay synchronized

### Replication Types

**Asynchronous Replication (default):**
- Primary doesn't wait for standby to confirm writes
- Better performance, but possible data loss if primary fails
- Standby might lag behind primary

**Synchronous Replication (our target):**
- Primary waits for standby to confirm WAL write
- Zero data loss (standby always has same data)
- Slight performance impact (wait for network + standby disk)
- Configured via `synchronous_commit` and `synchronous_standby_names`

---

## Architecture Design

### Current Architecture (Single Instance)

```
┌─────────────────────────────────────┐
│  Kubernetes Namespace: toygres      │
│                                     │
│  ┌───────────────────────────────┐  │
│  │  StatefulSet: mydb-abc123     │  │
│  │  Replicas: 1                  │  │
│  │  ┌─────────────────────────┐  │  │
│  │  │ Pod: mydb-abc123-0      │  │  │
│  │  │ - postgres:18           │  │  │
│  │  │ - PVC: mydb-abc123-pvc  │  │  │
│  │  └─────────────────────────┘  │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │  Service: mydb-abc123         │  │
│  │  Type: LoadBalancer           │  │
│  │  Port: 5432                   │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

### Proposed Architecture (Primary + Standby)

```
┌─────────────────────────────────────────────────────────────┐
│  Kubernetes Namespace: toygres                              │
│                                                             │
│  ┌──────────────────────────────────┐                       │
│  │  StatefulSet: mydb-abc123-primary│                       │
│  │  Replicas: 1                     │                       │
│  │  ┌────────────────────────────┐  │                       │
│  │  │ Pod: mydb-abc123-primary-0 │  │                       │
│  │  │ - postgres:18 (PRIMARY)    │  │◄──────┐              │
│  │  │ - PVC: mydb-abc123-pri-pvc │  │       │              │
│  │  │ - synchronous_commit=on    │  │       │ WAL Stream   │
│  │  └────────────────────────────┘  │       │              │
│  └──────────────────────────────────┘       │              │
│  ┌──────────────────────────────────┐       │              │
│  │ StatefulSet: mydb-abc123-standby │       │              │
│  │  Replicas: 1                     │       │              │
│  │  ┌────────────────────────────┐  │       │              │
│  │  │ Pod: mydb-abc123-standby-0 │  │───────┘              │
│  │  │ - postgres:18 (STANDBY)    │  │                      │
│  │  │ - PVC: mydb-abc123-stb-pvc │  │                      │
│  │  │ - recovery mode (read-only)│  │                      │
│  │  └────────────────────────────┘  │                      │
│  └──────────────────────────────────┘                      │
│                                                             │
│  ┌──────────────────────────────────┐                       │
│  │  Service: mydb-abc123 (primary)  │                       │
│  │  Type: LoadBalancer              │  ← User traffic      │
│  │  Selector: role=primary          │                       │
│  │  Port: 5432                      │                       │
│  └──────────────────────────────────┘                       │
│                                                             │
│  ┌──────────────────────────────────┐                       │
│  │  Service: mydb-abc123-standby    │                       │
│  │  Type: ClusterIP (internal only) │  ← Health checks     │
│  │  Selector: role=standby          │                       │
│  │  Port: 5432                      │                       │
│  └──────────────────────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

---

## PostgreSQL Configuration

### Primary Server Configuration

**postgresql.conf changes:**
```ini
# Replication settings
wal_level = replica                    # Enable replication
max_wal_senders = 3                    # Max concurrent standbys (2 standbys + 1 for backup)
wal_keep_size = 1024                   # Keep 1GB of WAL (PG 13+)
max_replication_slots = 3              # Replication slots for standbys

# Synchronous replication (CRITICAL for zero data loss)
synchronous_commit = on                # Wait for standby confirmation
synchronous_standby_names = 'mydb_standby'  # Name of standby that must confirm

# Archive settings (optional, for point-in-time recovery)
archive_mode = on
archive_command = 'test ! -f /archive/%f && cp %p /archive/%f'

# Hot standby (allow reads on standby)
hot_standby = on
```

**pg_hba.conf addition:**
```
# Allow replication connections from standby pod
host replication replicator 10.0.0.0/8 md5
```

**Create replication user:**
```sql
CREATE ROLE replicator WITH REPLICATION LOGIN ENCRYPTED PASSWORD '<replication-password>';
```

### Standby Server Configuration

**No postgresql.conf needed** (inherits from primary via pg_basebackup)

**standby.signal file** (empty file that marks server as standby):
```bash
touch /var/lib/postgresql/data/pgdata/standby.signal
```

**postgresql.auto.conf or postgresql.conf additions:**
```ini
# Connection to primary
primary_conninfo = 'host=mydb-abc123-primary-0.mydb-abc123-primary user=replicator password=<password> application_name=mydb_standby'

# Standby settings
hot_standby = on                       # Allow read queries
```

---

## Kubernetes Resources

### New Resources Needed

For each instance, we'll create:

1. **Primary StatefulSet** (existing, modified)
   - Name: `{instance-name}-primary`
   - Replicas: 1
   - Role label: `role=primary`
   - Init containers: Configure replication settings

2. **Standby StatefulSet** (NEW)
   - Name: `{instance-name}-standby`
   - Replicas: 1
   - Role label: `role=standby`
   - Init containers: Run pg_basebackup from primary

3. **Primary Service** (existing, modified)
   - Name: `{instance-name}`
   - Selector: `role=primary`
   - Type: LoadBalancer (public access)

4. **Standby Service** (NEW)
   - Name: `{instance-name}-standby`
   - Selector: `role=standby`
   - Type: ClusterIP (internal only)

5. **Headless Service for Replication** (NEW)
   - Name: `{instance-name}-primary` (for StatefulSet DNS)
   - clusterIP: None
   - Enables: `{instance-name}-primary-0.{instance-name}-primary.{namespace}.svc.cluster.local`

6. **Secrets** (NEW)
   - Name: `{instance-name}-replication`
   - Contains: Replication user password

7. **ConfigMaps** (NEW)
   - Name: `{instance-name}-primary-config`
   - Contains: Primary postgresql.conf overrides and pg_hba.conf

---

## Implementation Steps

### Step 1: Modify Primary Deployment

**Changes to primary StatefulSet:**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: {{ name }}-primary
  labels:
    role: primary
spec:
  serviceName: {{ name }}-primary  # Headless service for DNS
  replicas: 1
  template:
    metadata:
      labels:
        role: primary
    spec:
      initContainers:
      - name: configure-replication
        image: postgres:{{ postgres_version }}
        command:
        - bash
        - -c
        - |
          # Create replication user setup script
          cat > /docker-entrypoint-initdb.d/01-replication.sql <<EOF
          CREATE ROLE replicator WITH REPLICATION LOGIN ENCRYPTED PASSWORD '${REPLICATION_PASSWORD}';
          EOF
          
          # Add replication config to postgresql.conf
          cat >> /var/lib/postgresql/data/pgdata/postgresql.conf <<EOF
          # Replication settings
          wal_level = replica
          max_wal_senders = 3
          wal_keep_size = 1024
          max_replication_slots = 3
          synchronous_commit = on
          synchronous_standby_names = '{{ name }}_standby'
          hot_standby = on
          EOF
          
          # Add pg_hba.conf entry for replication
          echo "host replication replicator 0.0.0.0/0 md5" >> /var/lib/postgresql/data/pgdata/pg_hba.conf
        env:
        - name: REPLICATION_PASSWORD
          valueFrom:
            secretKeyRef:
              name: {{ name }}-replication
              key: password
        volumeMounts:
        - name: postgres-storage
          mountPath: /var/lib/postgresql/data
        - name: initdb
          mountPath: /docker-entrypoint-initdb.d
      
      containers:
      - name: postgres
        image: postgres:{{ postgres_version }}
        env:
        - name: POSTGRES_PASSWORD
          value: "{{ password }}"
        - name: POSTGRES_USER
          value: postgres
        volumeMounts:
        - name: postgres-storage
          mountPath: /var/lib/postgresql/data
      
      volumes:
      - name: postgres-storage
        persistentVolumeClaim:
          claimName: {{ name }}-primary-pvc
      - name: initdb
        emptyDir: {}
```

### Step 2: Create Standby StatefulSet

**New template: `postgres-standby-statefulset.yaml`:**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: {{ name }}-standby
  labels:
    role: standby
spec:
  serviceName: {{ name }}-standby
  replicas: 1
  template:
    metadata:
      labels:
        role: standby
    spec:
      initContainers:
      - name: clone-from-primary
        image: postgres:{{ postgres_version }}
        command:
        - bash
        - -c
        - |
          set -e
          
          # Wait for primary to be ready
          until pg_isready -h {{ name }}-primary-0.{{ name }}-primary.{{ namespace }}.svc.cluster.local -U postgres; do
            echo "Waiting for primary to be ready..."
            sleep 5
          done
          
          echo "Primary is ready, starting pg_basebackup..."
          
          # Create base backup from primary
          PGPASSWORD=${REPLICATION_PASSWORD} pg_basebackup \
            -h {{ name }}-primary-0.{{ name }}-primary.{{ namespace }}.svc.cluster.local \
            -U replicator \
            -D /var/lib/postgresql/data/pgdata \
            -P \
            -X stream \
            -R  # Automatically creates standby.signal and configures primary_conninfo
          
          # Additional standby configuration
          cat >> /var/lib/postgresql/data/pgdata/postgresql.auto.conf <<EOF
          hot_standby = on
          EOF
          
          echo "Base backup complete, standby configured"
        env:
        - name: REPLICATION_PASSWORD
          valueFrom:
            secretKeyRef:
              name: {{ name }}-replication
              key: password
        - name: PGDATA
          value: /var/lib/postgresql/data/pgdata
        volumeMounts:
        - name: postgres-storage
          mountPath: /var/lib/postgresql/data
      
      containers:
      - name: postgres
        image: postgres:{{ postgres_version }}
        ports:
        - containerPort: 5432
          name: postgres
        env:
        - name: PGDATA
          value: /var/lib/postgresql/data/pgdata
        volumeMounts:
        - name: postgres-storage
          mountPath: /var/lib/postgresql/data
        readinessProbe:
          exec:
            command:
            - pg_isready
            - -U
            - postgres
          initialDelaySeconds: 10
          periodSeconds: 5
  volumeClaimTemplates:
  - metadata:
      name: postgres-storage
    spec:
      accessModes: [ "ReadWriteOnce" ]
      storageClassName: default
      resources:
        requests:
          storage: {{ storage_size }}Gi
```

### Step 3: Services

**Headless Service for Primary (NEW):**
```yaml
apiVersion: v1
kind: Service
metadata:
  name: {{ name }}-primary
spec:
  clusterIP: None  # Headless
  selector:
    role: primary
    instance: {{ name }}
  ports:
  - port: 5432
    name: postgres
```

**Primary LoadBalancer Service (MODIFIED):**
```yaml
apiVersion: v1
kind: Service
metadata:
  name: {{ name }}
spec:
  type: LoadBalancer
  selector:
    role: primary    # Only route to primary
    instance: {{ name }}
  ports:
  - port: 5432
    targetPort: 5432
```

**Standby Service (NEW):**
```yaml
apiVersion: v1
kind: Service
metadata:
  name: {{ name }}-standby
spec:
  type: ClusterIP
  selector:
    role: standby
    instance: {{ name }}
  ports:
  - port: 5432
    name: postgres
```

---

## CMS Database Schema Changes

### Extend `instances` Table

```sql
ALTER TABLE toygres_cms.instances 
ADD COLUMN IF NOT EXISTS replication_enabled BOOLEAN NOT NULL DEFAULT false,
ADD COLUMN IF NOT EXISTS primary_k8s_name VARCHAR(255),
ADD COLUMN IF NOT EXISTS standby_k8s_name VARCHAR(255),
ADD COLUMN IF NOT EXISTS replication_status VARCHAR(50),  -- 'syncing', 'synchronized', 'lagging', 'broken'
ADD COLUMN IF NOT EXISTS replication_lag_bytes BIGINT,
ADD COLUMN IF NOT EXISTS last_replication_check_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_instances_replication_status 
    ON instances(replication_status) 
    WHERE replication_enabled = true;
```

### New Table: `replication_events`

```sql
CREATE TABLE IF NOT EXISTS toygres_cms.replication_events (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,  -- 'standby_created', 'sync_established', 'lag_detected', 'failover', 'promoted'
    primary_name VARCHAR(255),
    standby_name VARCHAR(255),
    lag_bytes BIGINT,
    message TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_replication_events_instance_id 
    ON replication_events(instance_id);
CREATE INDEX IF NOT EXISTS idx_replication_events_type 
    ON replication_events(event_type);
CREATE INDEX IF NOT EXISTS idx_replication_events_created_at 
    ON replication_events(created_at DESC);
```

---

## New Activities

### 1. `deploy_standby_postgres`

**Purpose**: Deploy standby PostgreSQL instance with replication configured

**Input:**
```rust
pub struct DeployStandbyInput {
    pub primary_instance_name: String,
    pub standby_instance_name: String,
    pub namespace: String,
    pub postgres_version: String,
    pub storage_size_gb: i32,
    pub replication_password: String,
}
```

**Steps:**
1. Create replication secret
2. Create headless service for primary (if not exists)
3. Deploy standby StatefulSet (uses pg_basebackup in init container)
4. Create standby service (ClusterIP)
5. Wait for standby to complete initial sync

**Output:**
```rust
pub struct DeployStandbyOutput {
    pub standby_k8s_name: String,
    pub sync_time_seconds: i32,
}
```

### 2. `wait_for_replication_sync`

**Purpose**: Poll standby until it's synchronized with primary

**Input:**
```rust
pub struct WaitForReplicationSyncInput {
    pub primary_instance_name: String,
    pub standby_instance_name: String,
    pub namespace: String,
    pub max_lag_bytes: i64,  // Consider synced if lag < this
}
```

**Steps:**
1. Connect to primary, query `pg_stat_replication`
2. Check standby sync_state (should be 'sync' or 'streaming')
3. Check replication lag (`sent_lsn - write_lsn`)
4. Return when lag is acceptable

**Output:**
```rust
pub struct WaitForReplicationSyncOutput {
    pub sync_state: String,
    pub lag_bytes: i64,
    pub synced: bool,
}
```

### 3. `configure_primary_for_replication`

**Purpose**: Update primary server's replication configuration

**Input:**
```rust
pub struct ConfigurePrimaryInput {
    pub instance_name: String,
    pub namespace: String,
    pub standby_name: String,
    pub replication_password: String,
}
```

**Steps:**
1. Connect to primary PostgreSQL
2. Create replication user: `CREATE ROLE replicator WITH REPLICATION LOGIN ENCRYPTED PASSWORD '...'`
3. Update `synchronous_standby_names` via ALTER SYSTEM
4. Reload configuration: `SELECT pg_reload_conf()`
5. Verify settings applied

### 4. `check_replication_status`

**Purpose**: Check replication health and lag

**Input:**
```rust
pub struct CheckReplicationStatusInput {
    pub primary_instance_name: String,
    pub namespace: String,
}
```

**Steps:**
1. Connect to primary
2. Query `pg_stat_replication` view:
   ```sql
   SELECT 
     application_name,
     state,
     sync_state,
     sent_lsn,
     write_lsn,
     flush_lsn,
     replay_lsn,
     write_lag,
     flush_lag,
     replay_lag
   FROM pg_stat_replication;
   ```
3. Calculate lag bytes
4. Return status

**Output:**
```rust
pub struct CheckReplicationStatusOutput {
    pub standby_connected: bool,
    pub sync_state: String,      // 'sync', 'async', 'potential'
    pub lag_bytes: i64,
    pub write_lag_ms: i64,
    pub healthy: bool,
}
```

### 5. `promote_standby_to_primary`

**Purpose**: Promote standby to primary (for failover scenarios)

**Input:**
```rust
pub struct PromoteStandbyInput {
    pub standby_instance_name: String,
    pub namespace: String,
}
```

**Steps:**
1. Connect to standby pod
2. Execute: `pg_ctl promote -D /var/lib/postgresql/data/pgdata`
3. Wait for promotion to complete
4. Update service selectors to route traffic to new primary
5. Update CMS database to reflect role change

**Output:**
```rust
pub struct PromoteStandbyOutput {
    pub promoted: bool,
    pub new_primary_name: String,
}
```

---

## Modified Orchestrations

### `CreateInstanceOrchestration` (Modified)

**New workflow:**

```rust
pub async fn create_instance_orchestration(
    ctx: OrchestrationContext,
    input: CreateInstanceInput,
) -> Result<CreateInstanceOutput, String> {
    // Step 1: Reserve CMS record + DNS name (existing)
    
    // Step 2: Deploy PRIMARY PostgreSQL with replication enabled
    let primary_name = format!("{}-primary", input.name);
    deploy_primary_with_replication(...).await?;
    
    // Step 3: Wait for primary to be ready (existing)
    wait_for_primary_ready(...).await?;
    
    // Step 4: Configure primary for replication
    configure_primary_for_replication(...).await?;
    
    // Step 5: Deploy STANDBY PostgreSQL
    let standby_name = format!("{}-standby", input.name);
    deploy_standby_postgres(...).await?;
    
    // Step 6: Wait for standby to sync
    wait_for_replication_sync(...).await?;
    
    // Step 7: Get connection strings (existing, but only from primary)
    
    // Step 8: Test connection (existing)
    
    // Step 9: Update CMS state with replication info
    update_instance_state_with_replication(...).await?;
    
    // Step 10: Start health monitor (existing)
    
    Ok(output)
}
```

### `DeleteInstanceOrchestration` (Modified)

**New workflow:**

```rust
pub async fn delete_instance_orchestration(
    ctx: OrchestrationContext,
    input: DeleteInstanceInput,
) -> Result<DeleteInstanceOutput, String> {
    // Step 1: Cancel health monitor (existing)
    
    // Step 2: Delete standby resources (NEW)
    delete_standby_statefulset(...).await?;
    delete_standby_service(...).await?;
    delete_standby_pvc(...).await?;
    
    // Step 3: Delete primary resources (existing)
    delete_primary_statefulset(...).await?;
    delete_primary_service(...).await?;
    delete_primary_pvc(...).await?;
    
    // Step 4: Free DNS name (existing)
    
    Ok(output)
}
```

### `HealthCheckOrchestration` (Modified)

**Enhanced to check replication:**

```rust
loop {
    // Existing: Check primary health
    let primary_health = check_primary_health(...).await?;
    
    // NEW: Check replication status
    if replication_enabled {
        let repl_status = check_replication_status(...).await?;
        
        if !repl_status.healthy {
            // Log warning, possibly trigger alert
            ctx.trace_warn(format!(
                "Replication unhealthy: lag={} bytes, state={}", 
                repl_status.lag_bytes, 
                repl_status.sync_state
            ));
        }
        
        // Update CMS with replication metrics
        update_replication_metrics(...).await?;
    }
    
    // Wait 30 seconds
    ctx.create_timer(Duration::from_secs(30)).await?;
}
```

---

## Failover Orchestration (NEW)

### `FailoverOrchestration`

**Purpose**: Promote standby to primary when primary fails

**Trigger**: Manual or automatic (when health checks detect primary failure)

**Workflow:**

```rust
pub async fn failover_orchestration(
    ctx: OrchestrationContext,
    input: FailoverInput,
) -> Result<FailoverOutput, String> {
    ctx.trace_info("Starting failover process");
    
    // Step 1: Verify primary is actually down
    let primary_health = check_primary_health(...).await?;
    if primary_health.healthy {
        return Err("Primary is healthy, failover not needed".to_string());
    }
    
    // Step 2: Promote standby to primary
    promote_standby_to_primary(...).await?;
    
    // Step 3: Update services to route to new primary
    update_service_selectors(...).await?;
    
    // Step 4: Update CMS database
    record_failover_event(...).await?;
    update_instance_primary_standby_names(...).await?;
    
    // Step 5: (Optional) Create new standby from new primary
    // This can be done in a separate orchestration to avoid blocking
    
    ctx.trace_info("Failover complete");
    
    Ok(FailoverOutput {
        old_primary: input.primary_name,
        new_primary: input.standby_name,
        failover_time_seconds: ...,
    })
}
```

---

## CLI Changes

### Create Command (Modified)

```bash
toygres create mydb --password secret123 [OPTIONS]

New Options:
  --no-replication         Disable standby replica (single instance)
  --replication-mode MODE  async|sync [default: sync]
```

### New Commands

```bash
# Check replication status
toygres replication status mydb

# Manually trigger failover
toygres replication failover mydb [--force]

# Promote standby to primary
toygres replication promote mydb-standby

# Re-sync broken replication
toygres replication resync mydb
```

### Get Command (Enhanced)

```bash
toygres get mydb

# New output section:
Replication:
  Enabled:            Yes
  Mode:               Synchronous
  Primary:            mydb-abc123-primary
  Standby:            mydb-abc123-standby
  Sync State:         Synchronized
  Lag:                0 bytes
  Last Check:         2 seconds ago
```

---

## Migration Strategy

### Phase 1: Proof of Concept (Manual Testing)

1. Deploy single instance with replication manually
2. Verify streaming replication works
3. Test synchronous commit behavior
4. Validate failover process

**Test manually in Kubernetes:**
```bash
# Create primary
kubectl apply -f primary-statefulset.yaml
kubectl apply -f primary-service.yaml

# Create standby
kubectl apply -f standby-statefulset.yaml
kubectl apply -f standby-service.yaml

# Verify replication
kubectl exec -it mydb-primary-0 -- psql -U postgres -c "SELECT * FROM pg_stat_replication;"
```

### Phase 2: Activities Implementation

1. Implement new activities:
   - `deploy_standby_postgres`
   - `wait_for_replication_sync`
   - `configure_primary_for_replication`
   - `check_replication_status`
   - `promote_standby_to_primary`

2. Test each activity independently

### Phase 3: Orchestration Integration

1. Modify `CreateInstanceOrchestration`
2. Modify `DeleteInstanceOrchestration`
3. Modify `HealthCheckOrchestration`
4. Add `FailoverOrchestration`

### Phase 4: CMS Schema Migration

1. Create migration: `0002_add_replication.sql`
2. Run migration on production database
3. Update activity types to include replication fields

### Phase 5: CLI Enhancements

1. Add replication options to create command
2. Add replication status to get command
3. Add new `replication` subcommand group

### Phase 6: Gradual Rollout

1. **Flag-based rollout**: Add `--enable-replication` flag (default: false)
2. **Test with new instances**: Create test instances with replication
3. **Monitor**: Watch for issues, replication lag, performance impact
4. **Enable by default**: Once stable, flip default to true
5. **Backfill**: Optionally add standbys to existing instances

---

## Key Decisions & Trade-offs

### 1. Synchronous vs Asynchronous

**Decision: Start with Synchronous**
- ✅ Zero data loss guarantee
- ✅ Better for production databases
- ❌ Slight latency increase (~1-5ms per transaction)
- ❌ Primary blocks if standby is down

**Mitigation:**
- Provide async mode as option for performance-sensitive workloads
- Use `synchronous_commit = remote_write` (faster than `on`)

### 2. Single Standby vs Multiple Standbys

**Decision: Single Standby Initially**
- ✅ Simpler to implement
- ✅ Lower resource cost (50% overhead vs 200%+)
- ✅ Meets HA requirements
- ❌ No read scaling
- ❌ Can't handle multiple failures

**Future:** Allow multiple standbys via `--standby-count` flag

### 3. Automatic Failover vs Manual

**Decision: Manual Failover Initially**
- ✅ Safer (no automatic data loss scenarios)
- ✅ Simpler implementation
- ✅ User has control
- ❌ Requires human intervention
- ❌ Higher RTO (Recovery Time Objective)

**Future:** Implement automatic failover with consensus (like Patroni)

### 4. Storage Location

**Decision: Each Pod Gets Own PVC**
- ✅ Isolation (standby can't corrupt primary data)
- ✅ Supports ReadWriteOnce volumes (most cloud providers)
- ✅ Easy to manage individually
- ❌ 2x storage cost
- ❌ More PVCs to manage

**Alternative Considered:** Shared PVC (requires ReadWriteMany, often slower)

### 5. Init Container vs Sidecar

**Decision: Init Container for pg_basebackup**
- ✅ Runs once at startup
- ✅ Cleaner pod lifecycle
- ✅ Standby stays as standby (doesn't need coordination)
- ❌ Longer startup time (must wait for base backup)

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_deploy_standby_activity() {
    // Test standby deployment activity
}

#[tokio::test]
async fn test_replication_sync_check() {
    // Test sync status checking
}
```

### Integration Tests

```bash
# 1. Create instance with replication
./toygres create testdb-repl --password test123

# 2. Verify both primary and standby are running
kubectl get pods -n toygres | grep testdb-repl

# 3. Check replication status
./toygres replication status testdb-repl

# 4. Write data to primary
psql -h testdb-repl... -c "CREATE TABLE test(id INT); INSERT INTO test VALUES (1);"

# 5. Verify data appears on standby (read-only)
psql -h testdb-repl-standby... -c "SELECT * FROM test;"

# 6. Test failover
./toygres replication failover testdb-repl

# 7. Verify standby is now primary and accepts writes
psql -h testdb-repl... -c "INSERT INTO test VALUES (2);"
```

### Performance Testing

1. **Benchmark with replication off**
2. **Benchmark with async replication**
3. **Benchmark with sync replication**
4. **Measure:** Throughput (TPS), latency (p50, p95, p99)

---

## Monitoring & Observability

### Metrics to Track

1. **Replication Lag**
   - Bytes behind
   - Time lag (write_lag, flush_lag, replay_lag)
   - Alert if lag > threshold

2. **Sync State**
   - 'sync' = synchronized (good)
   - 'potential' = potential for sync (transitioning)
   - 'async' = asynchronous (degraded)

3. **Standby Health**
   - Is standby connected?
   - Is standby applying WAL?
   - Last contact time

### Health Check Updates

```rust
// In HealthCheckOrchestration, add:

if replication_enabled {
    let repl_health = check_replication_status(...).await?;
    
    // Record metrics in CMS
    update_replication_metrics(
        lag_bytes: repl_health.lag_bytes,
        sync_state: repl_health.sync_state,
        healthy: repl_health.healthy,
    ).await?;
    
    // Alert if unhealthy
    if !repl_health.healthy {
        record_replication_event(
            event_type: "replication_unhealthy",
            message: format!("Lag: {} bytes", repl_health.lag_bytes),
        ).await?;
    }
}
```

---

## Resource Calculations

### Storage

**Before:** 1 instance = 1 PVC (10GB)
**After:** 1 instance = 2 PVCs (10GB primary + 10GB standby) = 20GB

**Cost impact:** 2x storage cost per instance

### Compute

**Before:** 1 instance = 1 pod
**After:** 1 instance = 2 pods (primary + standby)

**Cost impact:** 2x CPU/memory cost per instance

### Network

**Additional traffic:**
- WAL streaming: ~1-10 MB/s (depends on write load)
- Azure: Usually free within region

---

## Risks & Mitigations

### Risk 1: Standby Deployment Failure

**Scenario:** pg_basebackup fails (network issue, primary not ready)

**Mitigation:**
- Retry logic in init container
- Fall back to primary-only mode if standby fails
- User can retry standby creation manually

### Risk 2: Split-Brain

**Scenario:** Network partition causes both nodes to think they're primary

**Mitigation:**
- Use Kubernetes pod anti-affinity (schedule on different nodes)
- Fencing: Ensure only one pod can write
- Manual failover only (no automatic promotion initially)

### Risk 3: Performance Degradation

**Scenario:** Synchronous replication slows down writes

**Mitigation:**
- Use `synchronous_commit = remote_write` (not `on`)
- Make async mode available
- Benchmark and document performance impact
- Use faster inter-pod networking (same AZ)

### Risk 4: Storage Costs

**Scenario:** Doubling storage for every instance increases costs significantly

**Mitigation:**
- Make replication optional (`--no-replication` flag)
- Charge appropriately in pricing
- Use smaller storage for standby if primary isn't full

---

## Implementation Priority

### Phase 1: Core Replication (Essential)
1. ✅ Research and design (this document)
2. ⬜ Create new templates (primary, standby, services)
3. ⬜ Implement `deploy_standby_postgres` activity
4. ⬜ Implement `wait_for_replication_sync` activity
5. ⬜ Implement `configure_primary_for_replication` activity
6. ⬜ Test manually in Kubernetes

### Phase 2: Orchestration Integration
7. ⬜ Modify `CreateInstanceOrchestration`
8. ⬜ Modify `DeleteInstanceOrchestration`
9. ⬜ Add CMS schema migration
10. ⬜ Update activity types and inputs
11. ⬜ End-to-end testing

### Phase 3: Monitoring
12. ⬜ Implement `check_replication_status` activity
13. ⬜ Enhance `HealthCheckOrchestration`
14. ⬜ Add replication metrics to CMS
15. ⬜ Update `toygres get` to show replication info

### Phase 4: Failover
16. ⬜ Implement `promote_standby_to_primary` activity
17. ⬜ Create `FailoverOrchestration`
18. ⬜ Add CLI commands for failover
19. ⬜ Test failover scenarios

### Phase 5: Polish
20. ⬜ Add `--no-replication` flag
21. ⬜ Add async replication mode option
22. ⬜ Performance benchmarking
23. ⬜ Documentation and examples

---

## Open Questions

1. **Should replication be enabled by default?**
   - Pros: Better HA out of the box
   - Cons: 2x cost, longer deployment time
   - Recommendation: Start as opt-in, make default later

2. **What's the acceptable replication lag threshold?**
   - Sync replication: 0 bytes (by definition)
   - But network delays can cause temporary lag
   - Recommendation: Alert if lag > 10MB or > 10 seconds

3. **Should we support cascading replication?**
   - Primary → Standby1 → Standby2 → ...
   - Reduces load on primary
   - More complex
   - Recommendation: Not for v1

4. **How to handle standby that falls too far behind?**
   - Rebuild from scratch (new pg_basebackup)
   - Switch to async temporarily
   - Recommendation: Rebuild if lag > 1GB

5. **Should standby be in different availability zone?**
   - Better HA (survives AZ failure)
   - Higher latency (cross-AZ network)
   - Recommendation: Make configurable, default to same AZ

---

## Success Criteria

### Functional

- ✅ Primary and standby deploy successfully
- ✅ Streaming replication establishes automatically
- ✅ Synchronous mode confirms writes on standby
- ✅ Health checks monitor replication status
- ✅ Failover promotes standby to primary
- ✅ Delete cleans up both primary and standby

### Performance

- ✅ Synchronous replication adds < 10ms latency
- ✅ Standby lag stays < 1MB under normal load
- ✅ Deployment time < 5 minutes (including standby sync)

### Reliability

- ✅ Replication survives pod restarts
- ✅ Standby automatically reconnects after network issues
- ✅ Zero data loss during planned failover
- ✅ System remains available during standby failure

---

## References

- [PostgreSQL Replication Documentation](https://www.postgresql.org/docs/current/high-availability.html)
- [Streaming Replication](https://www.postgresql.org/docs/current/warm-standby.html#STREAMING-REPLICATION)
- [Synchronous Replication](https://www.postgresql.org/docs/current/warm-standby.html#SYNCHRONOUS-REPLICATION)
- [pg_basebackup](https://www.postgresql.org/docs/current/app-pgbasebackup.html)
- [Patroni (HA tool)](https://github.com/zalando/patroni) - for future automatic failover inspiration

---

## Next Steps

1. Review this plan
2. Decide on default behavior (replication on/off)
3. Create proof-of-concept: Deploy primary+standby manually in K8s
4. Implement Phase 1 activities
5. Integrate into orchestrations


