# Toygres CLI Design

## Overview

Design a comprehensive command-line interface for Toygres that provides the same feature set as the planned web UI, plus advanced debugging capabilities for Duroxide orchestrations. The CLI is the primary interface for automation, CI/CD, and power users.

---

## Design Philosophy

### Inspiration

Follow patterns from successful CLIs:
- **kubectl** - Kubernetes resource management
- **docker** - Container operations
- **gh** (GitHub CLI) - Modern, user-friendly
- **terraform** - Infrastructure management

### Principles

1. **Intuitive**: Commands follow natural language patterns
2. **Consistent**: Similar operations use similar syntax
3. **Composable**: Works well with pipes and scripts
4. **Informative**: Clear output with color coding
5. **Safe**: Confirmations for destructive operations
6. **Flexible**: Multiple output formats (table, JSON, YAML)

---

## Command Structure

### Top-Level Commands

```bash
toygres [COMMAND] [SUBCOMMAND] [FLAGS] [OPTIONS]
```

### Command Categories

```
Instance Management:
  create      Create a new PostgreSQL instance
  delete      Delete a PostgreSQL instance
  list        List all instances
  get         Get details of a specific instance
  
Health & Status:
  health      Show health status and history
  status      Show detailed instance status
  test        Test connection to an instance
  
Connection:
  connect     Get connection strings
  psql        Connect with psql (interactive)
  
Orchestration (Duroxide):
  orchestrations  Manage and inspect orchestrations
    list      List orchestrations
    get       Get orchestration details
    history   Show execution history
    cancel    Cancel a running orchestration
    retry     Retry a failed orchestration
  
System:
  stats       Show system statistics
  version     Show version information
  config      Manage CLI configuration
```

---

## Detailed Commands

### 1. Create Instance

```bash
toygres create [NAME] [OPTIONS]
```

**Examples**:
```bash
# Interactive mode (prompts for password, etc.)
toygres create proddb

# Full specification
toygres create proddb \
  --password "SecurePass123!" \
  --version 18 \
  --storage 20 \
  --public

# With custom namespace
toygres create testdb \
  --password "test123" \
  --namespace dev

# From file (future)
toygres create -f instance.yaml

# Non-interactive with env var password
PGPASSWORD="SecurePass123!" toygres create proddb --non-interactive
```

**Flags**:
```
-p, --password <PASSWORD>      PostgreSQL password (or use PGPASSWORD env var)
-v, --version <VERSION>        PostgreSQL version [default: 18]
-s, --storage <SIZE>           Storage size in GB [default: 10]
    --public                   Use LoadBalancer (public IP) [default]
    --internal                 Use ClusterIP (internal only)
-n, --namespace <NAMESPACE>    Kubernetes namespace [default: toygres]
-t, --tag <KEY=VALUE>          Add tags (can be used multiple times)
    --wait                     Wait for instance to be ready
    --timeout <DURATION>       Timeout for --wait [default: 10m]
-o, --output <FORMAT>          Output format: table|json|yaml [default: table]
    --non-interactive          Don't prompt for missing values
-f, --file <FILE>              Create from YAML file (future)
```

**Output**:
```
Creating PostgreSQL instance: proddb
✓ DNS name available: proddb.westus3.cloudapp.azure.com
✓ CMS record created
✓ Orchestration started: create-proddb-a1b2c3d4

Instance Details:
  Name:           proddb
  K8s Name:       proddb-a1b2c3d4
  DNS:            proddb.westus3.cloudapp.azure.com
  Version:        PostgreSQL 18
  Storage:        10 GB
  State:          creating
  Orchestration:  create-proddb-a1b2c3d4

Use 'toygres get proddb' to check status
Use 'toygres connect proddb' to get connection string
```

**Output (--output json)**:
```json
{
  "instance_id": "uuid",
  "user_name": "proddb",
  "k8s_name": "proddb-a1b2c3d4",
  "dns_name": "proddb",
  "state": "creating",
  "postgres_version": "18",
  "storage_size_gb": 10,
  "orchestration_id": "create-proddb-a1b2c3d4",
  "created_at": "2024-11-14T12:00:00Z"
}
```

**With --wait**:
```
Creating PostgreSQL instance: proddb
✓ DNS name available
✓ CMS record created
✓ Orchestration started

Waiting for instance to be ready...
⏳ Deploying Kubernetes resources... (30s)
⏳ Waiting for pod to start... (1m 15s)
⏳ Testing connection... (2m 30s)
✓ Instance ready! (2m 45s)

Connection String:
  postgresql://postgres:***@proddb.westus3.cloudapp.azure.com:5432/postgres

Instance is ready to use!
```

---

### 2. Delete Instance

```bash
toygres delete [NAME] [OPTIONS]
```

**Examples**:
```bash
# Interactive confirmation
toygres delete proddb

# Force delete (no confirmation)
toygres delete proddb --force

# Wait for deletion to complete
toygres delete proddb --wait

# Delete by K8s name
toygres delete proddb-a1b2c3d4 --by-k8s-name
```

**Flags**:
```
-f, --force                    Skip confirmation prompt
    --wait                     Wait for deletion to complete
    --timeout <DURATION>       Timeout for --wait [default: 5m]
    --by-k8s-name              Treat NAME as K8s name instead of DNS name
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output (with confirmation)**:
```
⚠️  Warning: This will permanently delete the PostgreSQL instance!

Instance: proddb
  K8s Name:  proddb-a1b2c3d4
  DNS:       proddb.westus3.cloudapp.azure.com
  Version:   PostgreSQL 18
  Created:   2 hours ago

This action cannot be undone!

? Are you sure you want to delete this instance? (y/N) █
```

**Output (after confirmation)**:
```
Deleting instance: proddb
✓ Health monitor stopped
✓ Kubernetes resources deleting
✓ CMS record marked as deleted

Orchestration ID: delete-proddb-a1b2c3d4

Use 'toygres orchestrations get delete-proddb-a1b2c3d4' to check status
```

---

### 3. List Instances

```bash
toygres list [OPTIONS]
toygres ls [OPTIONS]  # alias
```

**Examples**:
```bash
# List all instances
toygres list

# Filter by state
toygres list --state running

# Filter by health
toygres list --health unhealthy

# Search by name
toygres list --filter "prod*"

# JSON output
toygres list -o json

# Wide output (more columns)
toygres list --wide

# Watch mode (live updates)
toygres list --watch
```

**Flags**:
```
-s, --state <STATE>            Filter by state: creating|running|deleting|failed
-h, --health <HEALTH>          Filter by health: healthy|unhealthy|unknown
-f, --filter <PATTERN>         Filter by name pattern (glob)
-n, --namespace <NAMESPACE>    Filter by namespace
    --wide                     Show additional columns
-w, --watch                    Watch for changes (live updates)
-o, --output <FORMAT>          Output format: table|json|yaml|wide
```

**Output (table)**:
```
NAME        DNS NAME                       STATE     HEALTH     VERSION  AGE
proddb      proddb.westus3.cloudapp...     Running   ● Healthy  18       2h
testdb1     testdb1.westus3.cloudapp...    Creating  - Unknown  18       5m
analytics   analytics.westus3.cloudapp...  Running   ● Healthy  16       1d
olddb       olddb.westus3.cloudapp...      Running   ⚠ Unhealth 16       30d
devdb       devdb.westus3.cloudapp...      Failed    ❌ Failed  18       10m

5 instances found
```

**Output (--wide)**:
```
NAME      DNS NAME     STATE    HEALTH   VERSION  STORAGE  IP           CREATED              K8S NAME
proddb    proddb...    Running  Healthy  18       10GB     4.249.117.85 2024-11-14 12:00:00  proddb-a1b2c3d4
testdb1   testdb1...   Creating Unknown  18       10GB     -            2024-11-14 14:00:00  testdb1-b2c3d4e5
...
```

**Output (JSON)**:
```json
{
  "instances": [
    {
      "user_name": "proddb",
      "k8s_name": "proddb-a1b2c3d4",
      "dns_name": "proddb",
      "state": "running",
      "health_status": "healthy",
      "postgres_version": "18",
      "storage_size_gb": 10,
      "external_ip": "4.249.117.85",
      "created_at": "2024-11-14T12:00:00Z"
    },
    ...
  ],
  "total": 5
}
```

**Watch mode**:
```
Every 2s: toygres list

NAME        STATE     HEALTH     VERSION  AGE
proddb      Running   ● Healthy  18       2h 5m
testdb1     Creating  - Unknown  18       7m      ← updating live
...

^C to exit
```

---

### 4. Get Instance

```bash
toygres get [NAME] [OPTIONS]
```

**Examples**:
```bash
# Basic info
toygres get proddb

# JSON output
toygres get proddb -o json

# Show health history
toygres get proddb --show-health

# Continuous watch
toygres get proddb --watch
```

**Flags**:
```
    --show-health              Include health check history
-w, --watch                    Watch for changes
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output (table)**:
```
Instance: proddb
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Status:
  State:                Running
  Health:               ● Healthy (last check: 30 seconds ago)
  PostgreSQL Version:   18.0

Identity:
  User Name:            proddb
  K8s Name:             proddb-a1b2c3d4
  DNS Name:             proddb
  Instance ID:          550e8400-e29b-41d4-a716-446655440000

Configuration:
  Storage:              10 GB
  Networking:           LoadBalancer (Public IP)
  Namespace:            toygres

Network:
  External IP:          4.249.117.85
  DNS:                  proddb.westus3.cloudapp.azure.com
  Port:                 5432

Timestamps:
  Created:              2024-11-14 12:00:00 (2 hours ago)
  Updated:              2024-11-14 14:00:00 (just now)

Orchestrations:
  Create:               create-proddb-a1b2c3d4 (completed)
  Health Monitor:       health-proddb-a1b2c3d4 (running)

Tags:
  environment:          production
  team:                 backend
```

**Output (--show-health)**:
```
... (instance details above) ...

Health Check History (Last 10):
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

TIME                  STATUS    RESPONSE   VERSION
30 seconds ago        ● Healthy  142ms     PostgreSQL 18.0
1 minute ago          ● Healthy  138ms     PostgreSQL 18.0
1.5 minutes ago       ● Healthy  145ms     PostgreSQL 18.0
2 minutes ago         ● Healthy  140ms     PostgreSQL 18.0
2.5 minutes ago       ● Healthy  143ms     PostgreSQL 18.0
...
```

---

### 5. Health

```bash
toygres health [NAME] [OPTIONS]
```

**Examples**:
```bash
# Show current health and recent history
toygres health proddb

# Show last 50 checks
toygres health proddb --limit 50

# Only unhealthy checks
toygres health proddb --unhealthy-only

# Watch mode
toygres health proddb --watch

# Time range
toygres health proddb --since "1 hour ago"
toygres health proddb --since "2024-11-14 12:00:00"
```

**Flags**:
```
-l, --limit <N>                Show last N checks [default: 10]
    --since <TIME>             Show checks since time
    --unhealthy-only           Only show failed checks
-w, --watch                    Watch for new checks
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output**:
```
Health Status: proddb
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Current:     ● Healthy
Last Check:  30 seconds ago
Response:    142ms

Summary (Last 24 Hours):
  Total Checks:     2,880
  Healthy:          2,875 (99.8%)
  Unhealthy:        5 (0.2%)
  Avg Response:     145ms

Recent Checks:
TIME                  STATUS      RESPONSE  VERSION
2024-11-14 14:00:30  ● Healthy    142ms    PostgreSQL 18.0
2024-11-14 14:00:00  ● Healthy    138ms    PostgreSQL 18.0
2024-11-14 13:59:30  ● Healthy    145ms    PostgreSQL 18.0
2024-11-14 13:59:00  ● Healthy    140ms    PostgreSQL 18.0
2024-11-14 13:58:30  ⚠ Unhealthy  -        Connection timeout
2024-11-14 13:58:00  ● Healthy    143ms    PostgreSQL 18.0
...
```

---

### 6. Connect (Get Connection Strings)

```bash
toygres connect [NAME] [OPTIONS]
```

**Examples**:
```bash
# Show connection strings
toygres connect proddb

# Copy DNS connection string to clipboard
toygres connect proddb --copy

# Show password
toygres connect proddb --show-password

# Open interactive psql session
toygres connect proddb --psql

# Generate connection example for specific language
toygres connect proddb --example node
toygres connect proddb --example python
toygres connect proddb --example go
```

**Flags**:
```
    --copy                     Copy connection string to clipboard
    --show-password            Show password in connection string
    --psql                     Open interactive psql session
    --example <LANG>           Show connection example for language
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output (default)**:
```
Connection Strings: proddb
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

DNS (Recommended):
postgresql://postgres:***@proddb.westus3.cloudapp.azure.com:5432/postgres

IP Address:
postgresql://postgres:***@4.249.117.85:5432/postgres

Components:
  Host (DNS):  proddb.westus3.cloudapp.azure.com
  Host (IP):   4.249.117.85
  Port:        5432
  User:        postgres
  Database:    postgres

Use --show-password to reveal password
Use --copy to copy connection string
Use --psql to connect interactively
```

**Output (--show-password)**:
```
DNS Connection String:
postgresql://postgres:SecurePass123!@proddb.westus3.cloudapp.azure.com:5432/postgres

⚠️  Password visible! Don't share this output.
```

**Output (--example node)**:
```
Node.js Connection Example:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// Using pg (node-postgres)
const { Client } = require('pg');

const client = new Client({
  host: 'proddb.westus3.cloudapp.azure.com',
  port: 5432,
  user: 'postgres',
  password: process.env.PGPASSWORD,
  database: 'postgres',
});

await client.connect();
const res = await client.query('SELECT version()');
console.log(res.rows[0]);
await client.end();

// Or with connection string
const client = new Client({
  connectionString: 'postgresql://postgres:***@proddb.westus3.cloudapp.azure.com:5432/postgres'
});
```

---

### 7. Test Connection

```bash
toygres test [NAME] [OPTIONS]
```

**Examples**:
```bash
# Test connection
toygres test proddb

# Verbose output
toygres test proddb --verbose

# Custom query
toygres test proddb --query "SELECT count(*) FROM pg_stat_activity"
```

**Flags**:
```
-v, --verbose                  Show detailed connection info
-q, --query <SQL>              Custom query to run [default: SELECT version()]
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output**:
```
Testing connection: proddb
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Target:   proddb.westus3.cloudapp.azure.com:5432

Connecting... ✓ (145ms)
Querying...   ✓ (12ms)

Version: PostgreSQL 18.0 on x86_64-pc-linux-gnu

Connection: ● Successful
Total Time: 157ms
```

---

### 8. Orchestrations

```bash
toygres orchestrations [SUBCOMMAND] [OPTIONS]
toygres orch [SUBCOMMAND]  # alias
```

#### 8.1 List Orchestrations

```bash
toygres orchestrations list [OPTIONS]
```

**Examples**:
```bash
# List all orchestrations
toygres orchestrations list

# Filter by status
toygres orchestrations list --status running

# Filter by type
toygres orchestrations list --type create-instance

# Show orchestrations for specific instance
toygres orchestrations list --instance proddb

# Recent only
toygres orchestrations list --since "1 hour ago"
```

**Flags**:
```
-s, --status <STATUS>          Filter by status: running|completed|failed|cancelled
-t, --type <TYPE>              Filter by orchestration type
-i, --instance <NAME>          Filter by instance name
    --since <TIME>             Show orchestrations since time
-l, --limit <N>                Limit number of results [default: 20]
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output**:
```
Orchestrations
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

ID                              TYPE              STATUS     INSTANCE         STARTED
create-proddb-a1b2c3d4         create-instance   ✓ Completed proddb          2h ago
health-proddb-a1b2c3d4         health-monitor    ⟳ Running   proddb          2h ago
create-testdb1-b2c3d4e5        create-instance   ⟳ Running   testdb1         5m ago
delete-olddb-c3d4e5f6          delete-instance   ✓ Completed olddb           1d ago
create-faildb-d4e5f6g7         create-instance   ❌ Failed   faildb          10m ago

Use 'toygres orchestrations get <ID>' for details
```

#### 8.2 Get Orchestration Details

```bash
toygres orchestrations get [ORCHESTRATION_ID] [OPTIONS]
```

**Examples**:
```bash
# Basic details
toygres orchestrations get create-proddb-a1b2c3d4

# Show execution history
toygres orchestrations get create-proddb-a1b2c3d4 --history

# Show pending activities
toygres orchestrations get create-proddb-a1b2c3d4 --pending

# JSON output
toygres orchestrations get create-proddb-a1b2c3d4 -o json
```

**Flags**:
```
-h, --history                  Show execution history
-p, --pending                  Show pending activities and timers
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output (basic)**:
```
Orchestration: create-proddb-a1b2c3d4
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Status:          ✓ Completed
Type:            create-instance
Version:         1.0.0
Instance:        proddb (proddb-a1b2c3d4)

Timeline:
  Started:       2024-11-14 12:00:00 (2 hours ago)
  Completed:     2024-11-14 12:02:45
  Duration:      2m 45s

Input:
  user_name:     proddb
  name:          proddb-a1b2c3d4
  password:      *** (hidden)
  version:       18
  storage:       10 GB
  load_balancer: true

Output:
  instance_name: proddb-a1b2c3d4
  dns_name:      proddb.westus3.cloudapp.azure.com
  ip:            4.249.117.85

Execution Summary:
  Activities:    8 executed, 8 succeeded, 0 failed
  Retries:       0
  Timers:        1 (30s wait for pod ready)
```

**Output (--history)**:
```
... (basic details above) ...

Execution History:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

TIME                  EVENT                        DURATION  STATUS
2024-11-14 12:00:00  Orchestration Started         -         -
2024-11-14 12:00:01  cms-create-instance-record    0.3s      ✓
2024-11-14 12:00:02  deploy-postgres               1.2s      ✓
2024-11-14 12:00:03  wait-for-ready (attempt 1)    0.5s      ✓
2024-11-14 12:00:33  Timer: wait-30s               30s       ✓
2024-11-14 12:01:03  wait-for-ready (attempt 2)    0.5s      ✓
2024-11-14 12:01:33  Timer: wait-30s               30s       ✓
2024-11-14 12:02:03  wait-for-ready (attempt 3)    0.6s      ✓
2024-11-14 12:02:04  get-connection-strings        0.8s      ✓
2024-11-14 12:02:05  test-connection               0.2s      ✓
2024-11-14 12:02:06  cms-update-instance-state     0.3s      ✓
2024-11-14 12:02:07  start-health-monitor          0.1s      ✓
2024-11-14 12:02:08  cms-record-health-monitor     0.2s      ✓
2024-11-14 12:02:45  Orchestration Completed       -         ✓
```

**Output (--pending) for running orchestration**:
```
... (basic details) ...

Pending Activities:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Current:     wait-for-ready (attempt 15/60)
Started:     2024-11-14 14:10:00 (30s ago)

Pending Timers:
  wait-30s:  Fires in 5 seconds

Next Steps:
  1. Timer completes in 5s
  2. Retry wait-for-ready (attempt 16)
  3. Continue or schedule next timer
```

#### 8.3 Cancel Orchestration

```bash
toygres orchestrations cancel [ORCHESTRATION_ID] [OPTIONS]
```

**Examples**:
```bash
# Cancel orchestration
toygres orchestrations cancel create-testdb1-b2c3d4e5

# Force cancel (no confirmation)
toygres orchestrations cancel health-proddb-a1b2c3d4 --force
```

**Flags**:
```
-f, --force                    Skip confirmation
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output**:
```
⚠️  Cancel Orchestration: create-testdb1-b2c3d4e5

Type:      create-instance
Instance:  testdb1
Status:    Running
Started:   5 minutes ago

This will stop the orchestration. The instance may be in an incomplete state.

? Are you sure you want to cancel? (y/N) █

✓ Orchestration cancelled

The instance 'testdb1' may be partially created.
Use 'toygres get testdb1' to check status.
Use 'toygres delete testdb1' to clean up if needed.
```

#### 8.4 Orchestration History (System-wide)

```bash
toygres orchestrations history [OPTIONS]
```

**Examples**:
```bash
# Recent orchestrations
toygres orchestrations history

# Failed orchestrations
toygres orchestrations history --failed-only

# For specific instance
toygres orchestrations history --instance proddb

# Date range
toygres orchestrations history --since "2024-11-01" --until "2024-11-14"
```

**Flags**:
```
    --failed-only              Show only failed orchestrations
-i, --instance <NAME>          Filter by instance
    --since <TIME>             Start time
    --until <TIME>             End time
-l, --limit <N>                Limit results [default: 50]
-o, --output <FORMAT>          Output format: table|json|yaml
```

---

### 9. Stats (System Statistics)

```bash
toygres stats [OPTIONS]
```

**Examples**:
```bash
# System overview
toygres stats

# Watch mode (live updates)
toygres stats --watch

# JSON output
toygres stats -o json
```

**Flags**:
```
-w, --watch                    Watch for changes (live dashboard)
-o, --output <FORMAT>          Output format: table|json|yaml
```

**Output**:
```
Toygres System Statistics
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Instances:
  Total:          15
  Running:        12
  Creating:       2
  Deleting:       0
  Failed:         1

Health Status:
  Healthy:        11
  Unhealthy:      1
  Unknown:        3

Orchestrations (Last 24h):
  Total:          45
  Completed:      40
  Running:        3
  Failed:         2
  Cancelled:      0

Resource Usage:
  Storage:        180 GB (12 instances)
  Avg per inst:   15 GB

Recent Activity (Last Hour):
  Instances Created:   3
  Instances Deleted:   1
  Health Checks:       360
  Failed Checks:       2

System:
  CLI Version:    0.1.0
  API Server:     Connected (http://localhost:8080)
  Workers:        3 active
  Database:       Connected (PostgreSQL 18)

Last Updated: 2024-11-14 14:00:00 (just now)
```

---

### 10. Configuration

```bash
toygres config [SUBCOMMAND] [OPTIONS]
```

**Examples**:
```bash
# Show current configuration
toygres config get

# Set API endpoint
toygres config set api-url http://toygres.local:8080

# Set default namespace
toygres config set namespace production

# Set output format
toygres config set output json

# Reset to defaults
toygres config reset
```

**Configuration File**:
```yaml
# ~/.toygres/config.yaml

api:
  url: http://localhost:8080
  timeout: 30s

defaults:
  namespace: toygres
  output_format: table
  postgres_version: "18"
  storage_size_gb: 10

display:
  colors: true
  timestamps: relative  # or absolute
  timezone: local

authentication:  # future
  token: ""
  token_file: ~/.toygres/token
```

---

## Output Formats

### Table (Default)

Human-readable, colored, aligned columns.

### JSON

Machine-readable, perfect for parsing:
```bash
toygres list -o json | jq '.instances[] | select(.health_status == "unhealthy")'
```

### YAML

Human-readable structured format:
```bash
toygres get proddb -o yaml
```

### Wide

Table with additional columns (more info, less readable):
```bash
toygres list --wide
```

---

## Advanced Features

### 1. Shell Completion

```bash
# Generate completion script
toygres completion bash > ~/.toygres/completion.bash
toygres completion zsh > ~/.zshrc.d/toygres.zsh
toygres completion fish > ~/.config/fish/completions/toygres.fish

# Enables:
toygres get <TAB>     # Lists all instances
toygres delete prod<TAB>  # Completes to proddb
```

### 2. Aliases

Built-in short aliases:
```bash
toygres ls           # alias for list
toygres orch         # alias for orchestrations
toygres del          # alias for delete
```

### 3. Interactive Mode

```bash
# Start interactive shell
toygres shell

toygres> list
toygres> get proddb
toygres> delete testdb
toygres> exit
```

### 4. Scripting Support

Exit codes:
- 0: Success
- 1: General error
- 2: Invalid arguments
- 3: Not found
- 4: Orchestration failed
- 5: Timeout

```bash
#!/bin/bash
if toygres get proddb --health healthy; then
  echo "Database is healthy"
else
  echo "Database is unhealthy" >&2
  exit 1
fi
```

### 5. Verbose Mode

```bash
# Global verbose flag
toygres --verbose create proddb

# Shows:
# - API requests/responses
# - Timing information
# - Debug information
```

### 6. Dry Run

```bash
# Preview without executing
toygres create proddb --dry-run

# Output:
# Would create instance with:
#   Name: proddb
#   Version: 18
#   Storage: 10GB
#   LoadBalancer: true
#
# API Call (not executed):
#   POST http://localhost:8080/api/instances
#   { ... }
```

---

## Implementation Structure

### Crate: `toygres-cli`

```
toygres-cli/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point
    ├── cli.rs               # Command definitions (clap)
    ├── commands/
    │   ├── mod.rs
    │   ├── create.rs        # Create command
    │   ├── delete.rs        # Delete command
    │   ├── list.rs          # List command
    │   ├── get.rs           # Get command
    │   ├── health.rs        # Health command
    │   ├── connect.rs       # Connect command
    │   ├── test.rs          # Test command
    │   ├── orchestrations.rs # Orchestrations command group
    │   ├── stats.rs         # Stats command
    │   └── config.rs        # Config command
    ├── api/
    │   ├── mod.rs
    │   ├── client.rs        # HTTP client
    │   └── types.rs         # API request/response types
    ├── output/
    │   ├── mod.rs
    │   ├── table.rs         # Table formatter
    │   ├── json.rs          # JSON formatter
    │   └── yaml.rs          # YAML formatter
    ├── config.rs            # Configuration management
    ├── utils.rs             # Utilities (time, colors, etc.)
    └── lib.rs               # Shared library code
```

### Key Dependencies

```toml
[dependencies]
clap = { version = "4.0", features = ["derive", "color"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
colored = "2.0"
chrono = { version = "0.4", features = ["serde"] }
tabled = "0.15"           # Table formatting
indicatif = "0.17"        # Progress bars
dialoguer = "0.11"        # Interactive prompts
```

---

## CLI to Server Communication

### API Client

The CLI communicates with `toygres-server` via REST API:

```rust
// toygres-cli/src/api/client.rs
pub struct ToygressClient {
    base_url: String,
    client: reqwest::Client,
    timeout: Duration,
}

impl ToygressClient {
    pub async fn create_instance(&self, req: CreateInstanceRequest) -> Result<CreateInstanceResponse>;
    pub async fn delete_instance(&self, name: &str) -> Result<DeleteInstanceResponse>;
    pub async fn list_instances(&self, filters: ListFilters) -> Result<Vec<Instance>>;
    pub async fn get_instance(&self, name: &str) -> Result<Instance>;
    pub async fn get_health_history(&self, name: &str, limit: usize) -> Result<Vec<HealthCheck>>;
    pub async fn get_orchestration(&self, id: &str) -> Result<Orchestration>;
    pub async fn list_orchestrations(&self, filters: OrchestrationFilters) -> Result<Vec<Orchestration>>;
    pub async fn cancel_orchestration(&self, id: &str) -> Result<()>;
    pub async fn get_stats(&self) -> Result<SystemStats>;
}
```

### Server Endpoints Required

The API server needs these endpoints:

```
GET    /api/instances
POST   /api/instances
GET    /api/instances/:name
DELETE /api/instances/:name
GET    /api/instances/:name/health

GET    /api/orchestrations
GET    /api/orchestrations/:id
POST   /api/orchestrations/:id/cancel
GET    /api/orchestrations/:id/history

GET    /api/stats
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_create_command() {
        // Test CLI argument parsing
    }
    
    #[test]
    fn test_format_instance_table() {
        // Test output formatting
    }
}
```

### Integration Tests

```bash
# Test against real API
cargo test --test integration_tests -- --test-threads=1

# Test against mock API
cargo test --test cli_tests
```

### Manual Testing

```bash
# Test script
./scripts/test-cli.sh
```

---

## Questions for Review

1. **Command Naming**: Are the command names intuitive? Any better alternatives?
2. **Orchestration Inspection**: Is the level of detail appropriate for debugging?
3. **Interactive vs Non-Interactive**: Default behavior for scripts vs humans?
4. **Output Formats**: Need additional formats (CSV, etc.)?
5. **Shell Integration**: Priority for completion scripts?
6. **Error Messages**: How detailed should error messages be?

---

## Success Criteria

- ✅ All operations possible via CLI that web UI will have
- ✅ Deep orchestration introspection for debugging
- ✅ Script-friendly (JSON output, exit codes, non-interactive mode)
- ✅ Human-friendly (colors, progress bars, confirmations)
- ✅ Extensible for future features
- ✅ Well-documented with examples
- ✅ Fast (< 100ms for local operations)
- ✅ Reliable error handling and reporting

