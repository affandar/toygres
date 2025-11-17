# Toygres Admin Interface Plan (`toygres server` commands)

## Overview

The `toygres server` subcommand provides administrative and diagnostic capabilities for operators and advanced users. These commands are for system management, debugging, and monitoring - distinct from the simple user-facing commands (`create`, `list`, `get`, `delete`).

---

## Design Philosophy

### User Separation

**Regular Users** (simple, high-level):
```bash
toygres create mydb --password xxx    # Create database
toygres list                          # List databases
toygres get mydb                      # Check status
toygres delete mydb                   # Delete database
```

**Operators/Admins** (advanced, system-level):
```bash
toygres server start                  # Manage server
toygres server orchestrations         # Debug workflows
toygres server stats                  # System metrics
toygres server drift                  # Check CMS/K8s consistency
```

### Principles

1. **Non-destructive by default** - Read-only operations preferred
2. **Rich diagnostics** - Full visibility into system internals
3. **Scriptable** - JSON output for automation
4. **Progressive disclosure** - Basic info first, details on demand

---

## Command Structure

```
toygres server <subcommand> [options]
```

### Subcommands

```
Process Management:
  start           Start local development server
  stop            Stop local development server
  restart         Restart server
  status          Check if server is running
  logs            View server logs
  
Orchestrations (Duroxide Diagnostics):
  orchestrations  List orchestrations with filters
  orchestration   Get details of specific orchestration
  cancel          Cancel a running orchestration
  
System Health:
  stats           Show system statistics and metrics
  health          Check health of all system components
  workers         Show worker status and activity
  
Data Consistency:
  drift           Check for drift between CMS and K8s
  sync            Synchronize CMS with K8s state (reconciliation)
  
Database:
  db-status       Check CMS database health
  db-query        Run custom query on CMS database
  
Configuration:
  config          Show current configuration
  env             Show environment variables being used
```

---

## Command Specifications

### 1. Process Management (Already Implemented)

#### `toygres server start`
✅ Implemented

#### `toygres server stop`
✅ Implemented

#### `toygres server restart`
**New** - Convenience command

```bash
toygres server restart [OPTIONS]

# Equivalent to:
# toygres server stop && toygres server start
```

#### `toygres server status`
✅ Implemented

#### `toygres server logs`
✅ Implemented

---

### 2. Orchestrations (Duroxide Diagnostics)

#### `toygres server orchestrations`

List all orchestrations with rich filtering.

```bash
toygres server orchestrations [OPTIONS]

Options:
  -s, --status <STATUS>          Filter: running|completed|failed|cancelled
  -t, --type <TYPE>              Filter: create-instance|delete-instance|health-monitor
  -i, --instance <NAME>          Filter by instance name
      --since <TIME>             Show orchestrations since time
  -l, --limit <N>                Limit results [default: 20]
  -o, --output <FORMAT>          Output: table|json|yaml [default: table]
```

**Examples**:
```bash
# List recent orchestrations
toygres server orchestrations

# Show only running orchestrations
toygres server orchestrations --status running

# Show failed orchestrations
toygres server orchestrations --status failed

# Show orchestrations for specific instance
toygres server orchestrations --instance proddb

# Show create orchestrations only
toygres server orchestrations --type create-instance

# JSON output for scripting
toygres server orchestrations -o json | jq '.[] | select(.status == "failed")'
```

**Output (table)**:
```
Orchestrations
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

ID                              TYPE              STATUS      INSTANCE  STARTED      DURATION
create-proddb-a1b2c3d4         create-instance   ✓ Completed  proddb    2h ago       2m 45s
health-proddb-a1b2c3d4         health-monitor    ⟳ Running    proddb    2h ago       -
create-testdb1-b2c3d4e5        create-instance   ⟳ Running    testdb1   5m ago       -
delete-olddb-c3d4e5f6          delete-instance   ✓ Completed  olddb     1d ago       30s
create-faildb-d4e5f6g7         create-instance   ❌ Failed    faildb    10m ago      45s

5 orchestrations found

Use 'toygres server orchestration <ID>' for details
```

**Implementation**:
Query `toygres_duroxide` schema (duroxide tables) for orchestration state.

---

#### `toygres server orchestration <ID>`

Get detailed information about a specific orchestration.

```bash
toygres server orchestration [ID] [OPTIONS]

Options:
  -h, --history                  Show full execution history
  -p, --pending                  Show pending activities/timers
  -o, --output <FORMAT>          Output: table|json|yaml [default: table]
```

**Examples**:
```bash
# Basic details
toygres server orchestration create-proddb-a1b2c3d4

# Show execution history
toygres server orchestration create-proddb-a1b2c3d4 --history

# Show what's pending (for running orchestrations)
toygres server orchestration create-testdb1-b2c3d4e5 --pending

# JSON output
toygres server orchestration create-proddb-a1b2c3d4 -o json
```

**Output (basic)**:
```
Orchestration: create-proddb-a1b2c3d4
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Status:          ✓ Completed
Type:            create-instance
Version:         1.0.0
Instance:        proddb (proddb-a1b2c3d4)

Timeline:
  Started:       2024-11-17 12:00:00 (2 hours ago)
  Completed:     2024-11-17 12:02:45
  Duration:      2m 45s

Input:
  user_name:     proddb
  k8s_name:      proddb-a1b2c3d4
  dns_label:     proddb
  version:       18
  storage:       10 GB
  load_balancer: true

Output:
  dns_name:      proddb.westus3.cloudapp.azure.com
  external_ip:   4.249.117.85
  connection:    postgresql://postgres:***@proddb...

Execution Summary:
  Activities:    8 executed, 8 succeeded, 0 failed
  Timers:        2 (total wait: 60s)
  Retries:       0

Use '--history' to see full execution timeline
```

**Output (--history)**:
```
... (basic details above) ...

Execution History:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#  TIME                  EVENT                              DURATION  STATUS
1  2024-11-17 12:00:00  Orchestration Started               -         -
2  2024-11-17 12:00:01  cms-create-instance-record          0.3s      ✓
3  2024-11-17 12:00:02  deploy-postgres                     1.2s      ✓
4  2024-11-17 12:00:03  wait-for-ready                      0.5s      ✓ (not ready)
5  2024-11-17 12:00:33  Timer: wait-30s                     30s       ✓
6  2024-11-17 12:01:03  wait-for-ready                      0.5s      ✓ (not ready)
7  2024-11-17 12:01:33  Timer: wait-30s                     30s       ✓
8  2024-11-17 12:02:03  wait-for-ready                      0.6s      ✓ (ready!)
9  2024-11-17 12:02:04  get-connection-strings              0.8s      ✓
10 2024-11-17 12:02:05  test-connection                     0.2s      ✓
11 2024-11-17 12:02:06  cms-update-instance-state           0.3s      ✓
12 2024-11-17 12:02:07  start-health-monitor                0.1s      ✓
13 2024-11-17 12:02:08  cms-record-health-monitor           0.2s      ✓
14 2024-11-17 12:02:45  Orchestration Completed             -         ✓

Total: 8 activities, 2 timers, 2m 45s duration
```

**Output (--pending, for running orchestration)**:
```
... (basic details) ...

Current State:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Current Activity:
  wait-for-ready (attempt 15/60)
  Started: 2024-11-17 14:10:00 (30s ago)
  Expected: Pod is starting, may take several minutes

Pending Timers:
  wait-30s: Fires in 5 seconds

Next Steps:
  1. Timer completes in 5s
  2. Retry wait-for-ready (attempt 16/60)
  3. Continue or schedule next timer

Last 3 Events:
  12:09:00  wait-for-ready (attempt 14)  0.5s  ✓ (not ready)
  12:09:30  Timer: wait-30s               30s   ✓
  12:10:00  wait-for-ready (attempt 15)  0.5s  ⟳ In progress
```

---

#### `toygres server cancel <ID>`

Cancel a running orchestration.

```bash
toygres server cancel [ORCHESTRATION_ID] [OPTIONS]

Options:
  -f, --force                    Skip confirmation
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Examples**:
```bash
# Cancel with confirmation
toygres server cancel create-testdb1-b2c3d4e5

# Force cancel (no prompt)
toygres server cancel health-proddb-a1b2c3d4 --force
```

**Output**:
```
⚠️  Cancel Orchestration

ID:        create-testdb1-b2c3d4e5
Type:      create-instance
Instance:  testdb1
Status:    Running
Duration:  5 minutes

This will stop the orchestration immediately.
The instance may be in an incomplete state.

? Are you sure you want to cancel? (y/N) █

✓ Orchestration cancelled

Check instance state with: ./toygres get testdb1
```

---

### 3. System Health

#### `toygres server stats`

Show system-wide statistics and metrics.

```bash
toygres server stats [OPTIONS]

Options:
  -w, --watch                    Watch mode (refresh every 2s)
  -o, --output <FORMAT>          Output: table|json|yaml [default: table]
```

**Output**:
```
Toygres System Statistics
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Instances:
  Total:             15
  Running:           12  ████████████░░░  80%
  Creating:          2   ██░░░░░░░░░░░░░  13%
  Deleting:          0   ░░░░░░░░░░░░░░░   0%
  Failed:            1   █░░░░░░░░░░░░░░   7%

Health Status:
  Healthy:           11  ███████████░░░░  73%
  Unhealthy:         1   █░░░░░░░░░░░░░░   7%
  Unknown:           3   ██░░░░░░░░░░░░░  20%

Orchestrations (Last 24h):
  Total Started:     45
  Completed:         40  (88.9%)
  Running:           3   (6.7%)
  Failed:            2   (4.4%)
  Cancelled:         0   (0.0%)

By Type (Last 24h):
  create-instance:   20 started, 18 completed, 2 running
  delete-instance:   10 started, 10 completed
  health-monitor:    15 running

Activities (Last 24h):
  Total Executed:    380
  Succeeded:         375  (98.7%)
  Failed:            5    (1.3%)
  Avg Duration:      1.2s
  
  Slowest:
    get-connection-strings:  4.5s
    wait-for-ready:          3.2s
    deploy-postgres:         2.1s

Resource Usage:
  Storage (provisioned):  180 GB across 12 instances
  Average per instance:   15 GB

Workers:
  Active:            3
  Idle:              0
  Processing:        3 activities

Database:
  Status:            Connected
  Pool Size:         5 connections
  Active Queries:    2

Last Updated: 2024-11-17 14:00:00 (just now)
```

---

#### `toygres server health`

Deep health check of all system components.

```bash
toygres server health [OPTIONS]

Options:
  -v, --verbose                  Show detailed checks
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Output**:
```
System Health Check
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Control Plane:
  ✓ API Server             http://localhost:8080
  ✓ Duroxide Runtime       3 workers active
  ✓ CMS Database           PostgreSQL (toygres_cms schema)
  ✓ Duroxide Database      PostgreSQL (toygres_duroxide schema)

Kubernetes:
  ✓ Cluster Connection     toygres-aks (westus3)
  ✓ Namespace Access       toygres
  ✓ Storage Class          default (available)
  ✓ RBAC Permissions       create/read/update/delete pods, services, pvcs

Data Plane:
  ✓ Running Instances      12 healthy, 1 unhealthy
  ✓ Failed Instances       0 (no action needed)
  ✓ Orphaned Resources     0 (K8s resources without CMS record)
  ✓ Abandoned Records      0 (CMS records without K8s resources)

Overall Status: ✓ Healthy

Use '--verbose' for detailed checks
```

**Output (--verbose)**:
```
... (summary above) ...

Detailed Checks:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Control Plane:
  ✓ API Server
    - URL: http://localhost:8080
    - Response Time: 15ms
    - Endpoints: 6 available
  
  ✓ Duroxide Runtime
    - Workers: 3 active, 0 idle
    - Queue Size: 2 pending items
    - Processing: 2 activities
  
  ✓ CMS Database
    - Host: duroxide-pg-westus.postgres.database.azure.com
    - Schema: toygres_cms
    - Tables: 4 (instances, instance_events, instance_health_checks, drift_detections)
    - Connections: 2/5 active
    - Query Time: 12ms

Kubernetes:
  ✓ Cluster: toygres-aks
    - Region: westus3
    - Nodes: 3 (all ready)
    - Version: 1.28.3
  
  ✓ Namespace: toygres
    - StatefulSets: 12
    - Services: 12  
    - PVCs: 12
    - Pods: 12 running

Data Plane Instances:
  ✓ proddb (proddb-a1b2c3d4)
    - State: running / Health: healthy
    - K8s: StatefulSet ready, Pod running, Service has external IP
    - CMS: Record up-to-date, health monitor active
  
  ⚠ olddb (olddb-c3d4e5f6)
    - State: running / Health: unhealthy
    - K8s: Pod running, Service active
    - CMS: Last health check failed (Connection timeout)
    - Action: Investigate pod logs or database issues
  
  ... (continue for all instances) ...

Recommendations:
  ⚠ Instance 'olddb' is unhealthy - investigate with:
      ./toygres get olddb
      kubectl logs -n toygres olddb-c3d4e5f6-0
```

---

#### `toygres server workers`

Show worker status and current activity.

```bash
toygres server workers [OPTIONS]

Options:
  -w, --watch                    Watch mode (live updates)
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Output**:
```
Duroxide Workers
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

WORKER ID    STATUS     CURRENT ACTIVITY               DURATION  INSTANCE
work-12345   Active     deploy-postgres                 1.2s     proddb-a1b2c3d4
work-67890   Active     wait-for-ready                  0.5s     testdb1-b2c3d4e5
work-abcde   Active     cms-update-instance-state       0.3s     olddb-c3d4e5f6

3 workers active, 0 idle

Recent Completions (Last 10):
  work-12345:  test-connection (0.2s)  ✓
  work-67890:  get-connection-strings (0.8s)  ✓
  work-abcde:  delete-postgres (2.1s)  ✓
  ...

Queue:
  Pending Activities:  2
  Pending Timers:      5
```

---

### 4. Data Consistency (Drift Detection)

#### `toygres server drift`

Check for inconsistencies between CMS database and Kubernetes state.

```bash
toygres server drift [OPTIONS]

Options:
      --auto-resolve             Automatically resolve simple drifts
  -v, --verbose                  Show details for each drift
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Output (no drift)**:
```
Drift Detection
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Checking 12 instances...

✓ No drift detected

All CMS records match Kubernetes resources.
All Kubernetes resources have corresponding CMS records.
```

**Output (drift found)**:
```
Drift Detection
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Checking 12 instances...

⚠️ 2 drifts detected

1. Orphaned K8s Resources
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   K8s Name:     abandoned-db-xyz123
   Namespace:    toygres
   Resources:    StatefulSet, Service, PVC (all present)
   Issue:        No CMS record found
   
   Possible Causes:
   - Instance was created outside of Toygres
   - CMS database was restored from old backup
   - Manual deletion of CMS record
   
   Resolution Options:
   - Import to CMS:    toygres server sync --import abandoned-db-xyz123
   - Delete from K8s:  kubectl delete statefulset,svc,pvc -n toygres -l instance=abandoned-db-xyz123

2. Abandoned CMS Records
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Instance:     ghost-db
   K8s Name:     ghost-db-abc456
   State:        running
   Issue:        No K8s resources found (StatefulSet missing)
   
   Possible Causes:
   - Resources manually deleted from K8s
   - Kubernetes namespace was deleted
   - Cluster was recreated
   
   Resolution Options:
   - Mark as deleted:  toygres server sync --mark-deleted ghost-db
   - Recreate:         Not supported (would need to delete and recreate)

Drift Summary:
  Orphaned K8s:      1
  Abandoned CMS:     1
  State Mismatch:    0

Run with --auto-resolve to automatically fix simple issues.
Use './toygres server sync' for manual reconciliation.
```

---

#### `toygres server sync`

Reconcile state between CMS and Kubernetes.

```bash
toygres server sync [OPTIONS]

Options:
      --import <K8S_NAME>        Import orphaned K8s resource to CMS
      --mark-deleted <NAME>      Mark abandoned CMS record as deleted
      --dry-run                  Show what would be done without doing it
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Examples**:
```bash
# Dry run (see what would be done)
toygres server sync --dry-run

# Import orphaned K8s resource
toygres server sync --import abandoned-db-xyz123

# Mark CMS record as deleted
toygres server sync --mark-deleted ghost-db
```

---

### 5. Database

#### `toygres server db-status`

Check CMS database health and statistics.

```bash
toygres server db-status [OPTIONS]

Options:
  -v, --verbose                  Show detailed info
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Output**:
```
CMS Database Status
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Connection:
  ✓ Connected
  Host:     duroxide-pg-westus.postgres.database.azure.com
  Port:     5432
  Database: postgres
  User:     affandar

Schemas:
  ✓ toygres_cms        (4 tables, 180 MB)
  ✓ toygres_duroxide   (6 tables, 45 MB)

Tables (toygres_cms):
  instances                  15 rows   (5 MB)
  instance_events            89 rows   (2 MB)
  instance_health_checks  2,880 rows  (150 MB)
  drift_detections            0 rows   (0 MB)

Connection Pool:
  Max Connections:   5
  Active:            2
  Idle:              3

Performance:
  Avg Query Time:    15ms
  Slow Queries:      0 (> 1s)

Disk Usage:
  Total:             225 MB
  Available:         475 GB
```

---

#### `toygres server db-query`

Run custom SQL query on CMS database (advanced).

```bash
toygres server db-query [SQL] [OPTIONS]

Options:
  -f, --file <FILE>              Read SQL from file
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Examples**:
```bash
# Query instances
toygres server db-query "SELECT user_name, state FROM toygres_cms.instances"

# From file
toygres server db-query -f queries/health-summary.sql

# JSON output
toygres server db-query "SELECT * FROM toygres_cms.instances" -o json
```

**Output**:
```
Query Results
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

user_name    state
━━━━━━━━━━━━━━━━━━━━━━━━
proddb       running
testdb1      creating
analytics    running
...

15 rows returned (12ms)
```

---

### 6. Configuration

#### `toygres server config`

Show current server configuration.

```bash
toygres server config [OPTIONS]

Options:
  -o, --output <FORMAT>          Output: table|json|yaml [default: table]
```

**Output**:
```
Server Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Server:
  Mode:              standalone
  API Port:          8080
  Workers:           3

Database:
  URL:               postgresql://affandar@duroxide-pg-westus...
  Schema (CMS):      toygres_cms
  Schema (Duroxide): toygres_duroxide

Kubernetes:
  Cluster:           toygres-aks
  Resource Group:    adar-rg
  Namespace:         toygres
  Kubeconfig:        ~/.kube/config

Paths:
  PID File:          ~/.toygres/server.pid
  Log File:          ~/.toygres/server.log

Environment Variables:
  DATABASE_URL:        ✓ Set
  AKS_CLUSTER_NAME:    ✓ Set (toygres)
  AKS_RESOURCE_GROUP:  ✓ Set (adar-rg)
  AKS_NAMESPACE:       ✓ Set (toygres)
  RUST_LOG:            info
```

---

#### `toygres server env`

Show environment variables being used (debugging).

```bash
toygres server env

Options:
      --show-secrets             Show actual values (use with caution)
  -o, --output <FORMAT>          Output: table|json [default: table]
```

**Output**:
```
Environment Variables
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Required:
  DATABASE_URL           ✓ Set (***hidden***)
  AKS_CLUSTER_NAME       ✓ Set (toygres)
  AKS_RESOURCE_GROUP     ✓ Set (adar-rg)

Optional:
  AKS_NAMESPACE          ✓ Set (toygres)
  RUST_LOG               ✓ Set (info)
  TOYGRES_API_URL        ✗ Not set (using default: http://localhost:8080)

Computed:
  HOME                   /Users/affandar
  KUBECONFIG             ~/.kube/config (exists: yes)

Use --show-secrets to reveal hidden values (not recommended)
```

---

## Implementation Priority

### Phase 1: Essential Diagnostics (Now)
- ✅ start/stop/status/logs
- ✅ orchestrations (stub)
- ✅ orchestration <id> (stub)
- ⬜ cancel <id>
- ⬜ stats
- ⬜ config/env

### Phase 2: Health & Consistency
- ⬜ health
- ⬜ workers
- ⬜ drift
- ⬜ sync

### Phase 3: Advanced Features
- ⬜ db-status
- ⬜ db-query
- ⬜ restart
- ⬜ Watch modes for stats/workers

---

## API Endpoints Needed

To support these commands, the API server needs:

```
# Orchestrations
GET  /api/server/orchestrations
GET  /api/server/orchestrations/:id
GET  /api/server/orchestrations/:id/history
POST /api/server/orchestrations/:id/cancel

# System
GET  /api/server/stats
GET  /api/server/health
GET  /api/server/workers
GET  /api/server/config

# Drift Detection
GET  /api/server/drift
POST /api/server/sync

# Database
GET  /api/server/db-status
POST /api/server/db-query
```

---

## Data Sources

### CMS Database (PostgreSQL)
- Instance state and metadata
- Health check history
- Event logs
- Drift detection records

### Duroxide Database (PostgreSQL)
- Orchestration instances
- Execution history
- Activity execution records
- Timer state

### Kubernetes API
- StatefulSets, Services, PVCs
- Pod status
- Node information
- Resource quotas

### Duroxide Runtime (In-Memory)
- Worker status
- Current activity execution
- Queue sizes
- Performance metrics

---

## Security Considerations

### Sensitive Data

**Hidden by default:**
- Database passwords
- Connection strings with passwords
- API tokens (future)

**Shown with flags:**
```bash
# Reveal passwords
toygres get mydb --show-password

# Reveal all secrets
toygres server env --show-secrets
```

### Dangerous Operations

**Require confirmation:**
- `toygres server cancel` - Stops running orchestration
- `toygres server sync --mark-deleted` - Modifies CMS
- `toygres delete` - Deletes instance

**Skip confirmation:**
```bash
toygres delete mydb --force
toygres server cancel <id> --force
```

---

## Testing Strategy

### Manual Testing

```bash
# Test process management
./toygres server start
./toygres server status
./toygres server logs --tail 20
./toygres server stop

# Test diagnostics
./toygres server stats
./toygres server orchestrations
./toygres server health

# Test with running operations
./toygres create testdb --password test123
./toygres server orchestrations --status running
./toygres server orchestration <id> --history
```

### Integration Tests

```rust
#[tokio::test]
async fn test_server_start_stop() {
    // Start server
    // Check status
    // Stop server
    // Verify stopped
}

#[tokio::test]
async fn test_orchestration_list() {
    // Create instance
    // List orchestrations
    // Verify orchestration appears
}
```

---

## Questions for Review

1. **Drift Detection**: Should auto-resolve be automatic or require explicit flag?
2. **Database Query**: Should we restrict to SELECT only for safety?
3. **Worker Management**: Need ability to manually trigger work? (Probably not)
4. **Performance Metrics**: Should we add Prometheus export?
5. **Log Levels**: Should there be `toygres server logs --level error`?

---

## Success Criteria

- ✅ Complete visibility into system state
- ✅ Easy debugging of orchestration failures
- ✅ Drift detection and resolution
- ✅ Rich statistics and metrics
- ✅ All operations scriptable (JSON output)
- ✅ Safe defaults (confirmations, hidden secrets)
- ✅ Fast response times (< 1s for most commands)

