# Toygres Server Architecture

## Overview

Design the operational model for toygres-server, including how it runs, how the CLI interacts with it, and deployment patterns for different environments (local dev, testing, production).

---

## Server Modes

### Three Operational Modes

```rust
// toygres-server/src/main.rs
enum ServerMode {
    /// Combined API + Worker (all-in-one, default for dev)
    Standalone {
        api_port: u16,
        worker_threads: usize,
    },
    
    /// API server only (HTTP endpoints, no workers)
    Api {
        api_port: u16,
    },
    
    /// Worker only (processes orchestrations, no HTTP)
    Worker {
        worker_id: Option<String>,
    },
}
```

### Mode Comparison

| Mode | Use Case | Runs API | Runs Workers | Scales |
|------|----------|----------|--------------|--------|
| **Standalone** | Local dev, testing | ✓ | ✓ | Single instance |
| **API** | Production API pods | ✓ | ✗ | Horizontal (many replicas) |
| **Worker** | Production workers | ✗ | ✓ | Horizontal (many replicas) |

---

## CLI Interaction Models

### Model 1: External Server (Production)

**Scenario**: Production deployment on AKS

```
┌─────────────┐         HTTP/REST          ┌─────────────────┐
│             │ ──────────────────────────► │  toygres-server │
│ toygres CLI │                             │   (API mode)    │
│  (client)   │ ◄────────────────────────── │                 │
└─────────────┘                             └─────────────────┘
                                                     │
                                                     │ Duroxide Client
                                                     ▼
                                            ┌─────────────────┐
                                            │  toygres-server │
                                            │  (Worker mode)  │
                                            └─────────────────┘
```

**CLI Configuration**:
```bash
# Point CLI to production API
toygres config set api-url https://api.toygres.io
toygres config set api-token <token>

# Use CLI
toygres create proddb --password xxx
toygres list
```

**Server runs independently** - CLI cannot start/stop it.

---

### Model 2: Embedded Server (Local Development)

**Scenario**: Local development and testing

```
┌──────────────────────────────────────────┐
│           toygres CLI                     │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │  Can start/stop embedded server    │ │
│  │  Can tail server logs              │ │
│  └────────────────────────────────────┘ │
│                │                         │
│                ▼                         │
│  ┌────────────────────────────────────┐ │
│  │  Embedded toygres-server           │ │
│  │  (Standalone mode)                 │ │
│  └────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

**Usage**:
```bash
# Start server in background
toygres server start --background

# Start server in foreground (shows logs)
toygres server start

# Check if server is running
toygres server status

# Tail server logs
toygres server logs --follow

# Stop server
toygres server stop

# Restart server
toygres server restart
```

**CLI auto-detects**:
1. Check if external API configured → use it
2. Else check if local server running → use it
3. Else offer to start local server

---

## Recommended Approach: Hybrid Model

### Production: External Server

```yaml
# k8s/server/api-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: toygres-api
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: api
        image: toygres-server:latest
        args: ["api", "--port", "8080"]
        ports:
        - containerPort: 8080

---
# k8s/server/worker-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: toygres-workers
spec:
  replicas: 5
  template:
    spec:
      containers:
      - name: worker
        image: toygres-server:latest
        args: ["worker"]
```

**CLI Usage**:
```bash
# Configure once
toygres config set api-url https://api.toygres.io

# Use CLI
toygres create proddb --password xxx
toygres list
```

---

### Local Development: Embedded Server

**Auto-Start Behavior**:

```bash
# First time use - no server running
$ toygres list

⚠️  No toygres server found.

Options:
  1. Start local server:     toygres server start
  2. Connect to remote:      toygres config set api-url <url>

Would you like to start a local server? (Y/n) y

Starting toygres server (standalone mode)...
✓ Server started on http://localhost:8080 (PID: 12345)
✓ Workers initialized (1 worker thread)
✓ Connected to database

NAME        STATE     HEALTH     VERSION  AGE
(no instances found)

Server is running in background.
Use 'toygres server stop' to stop it.
Use 'toygres server logs' to view logs.
```

**Manual Control**:
```bash
# Start server explicitly
toygres server start

# Start with custom options
toygres server start --port 8080 --workers 2 --log-level debug

# Start in foreground (blocks, shows logs)
toygres server start --foreground

# Status check
toygres server status
# Output:
# ✓ Server running
#   PID:      12345
#   URL:      http://localhost:8080
#   Mode:     standalone
#   Workers:  2
#   Uptime:   2 hours 15 minutes

# View logs
toygres server logs --follow --tail 100

# Stop server
toygres server stop
```

---

## Server Command Structure

### `toygres server` Subcommands

```bash
toygres server start [OPTIONS]      # Start server
toygres server stop [OPTIONS]       # Stop server
toygres server restart [OPTIONS]    # Restart server
toygres server status              # Check if running
toygres server logs [OPTIONS]      # View logs
```

### Server Start Options

```bash
toygres server start [OPTIONS]

Options:
  -p, --port <PORT>              API port [default: 8080]
  -w, --workers <N>              Number of worker threads [default: 1]
      --mode <MODE>              Server mode: standalone|api|worker [default: standalone]
  -d, --background               Run in background (daemonize)
  -f, --foreground               Run in foreground (default)
      --log-level <LEVEL>        Log level: debug|info|warn|error [default: info]
      --log-file <FILE>          Log to file instead of stdout
      --pid-file <FILE>          PID file location [default: ~/.toygres/server.pid]
```

**Examples**:
```bash
# Start default (foreground, standalone)
toygres server start

# Start as background daemon
toygres server start --background

# Start with custom settings
toygres server start \
  --port 8080 \
  --workers 3 \
  --log-level debug \
  --background

# Start API only (for production split)
toygres server start --mode api --port 8080

# Start worker only (for production split)
toygres server start --mode worker
```

### Server Logs

```bash
toygres server logs [OPTIONS]

Options:
  -f, --follow                   Follow log output (like tail -f)
  -n, --tail <N>                 Show last N lines [default: 100]
      --since <TIME>             Show logs since time
      --level <LEVEL>            Filter by log level
      --grep <PATTERN>           Filter by pattern
  -o, --output <FORMAT>          Output format: text|json
```

**Examples**:
```bash
# View last 100 lines
toygres server logs

# Follow live logs
toygres server logs --follow

# Filter by log level
toygres server logs --level error

# Search logs
toygres server logs --grep "orchestration"

# Time range
toygres server logs --since "1 hour ago"
```

**Output**:
```
2024-11-14 14:00:00 [INFO]  toygres_server: Server started on 0.0.0.0:8080
2024-11-14 14:00:01 [INFO]  duroxide::runtime: Duroxide runtime initialized
2024-11-14 14:00:01 [INFO]  duroxide::worker: Worker started: work-12345
2024-11-14 14:05:23 [INFO]  toygres_server: Creating instance: proddb
2024-11-14 14:05:24 [INFO]  duroxide: Orchestration started: create-proddb-a1b2c3d4
2024-11-14 14:05:25 [INFO]  duroxide::activity: Deploy postgres: proddb-a1b2c3d4
2024-11-14 14:07:45 [INFO]  duroxide: Orchestration completed: create-proddb-a1b2c3d4
```

---

## Implementation Details

### Server Process Management

**File**: `toygres-cli/src/commands/server.rs`

```rust
pub struct ServerManager {
    pid_file: PathBuf,
    log_file: PathBuf,
    config: ServerConfig,
}

impl ServerManager {
    /// Start server process
    pub async fn start(&self, options: StartOptions) -> Result<()> {
        // Check if already running
        if self.is_running()? {
            return Err(anyhow!("Server already running (PID: {})", self.get_pid()?));
        }
        
        if options.background {
            self.start_background(options)?;
        } else {
            self.start_foreground(options)?;
        }
    }
    
    /// Start as background daemon
    fn start_background(&self, options: StartOptions) -> Result<()> {
        use std::process::{Command, Stdio};
        
        let child = Command::new("toygres-server")
            .args(self.build_args(&options))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        // Save PID
        std::fs::write(&self.pid_file, child.id().to_string())?;
        
        // Redirect output to log file
        self.setup_log_redirection(child)?;
        
        Ok(())
    }
    
    /// Stop server
    pub fn stop(&self) -> Result<()> {
        let pid = self.get_pid()?;
        
        // Send SIGTERM
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGTERM,
        )?;
        
        // Wait for graceful shutdown (up to 30s)
        for _ in 0..30 {
            if !self.is_running()? {
                std::fs::remove_file(&self.pid_file)?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        
        // Force kill if still running
        Err(anyhow!("Server did not stop gracefully"))
    }
    
    /// Check if server is running
    pub fn is_running(&self) -> Result<bool> {
        if let Some(pid) = self.get_pid().ok() {
            // Check if process exists
            Ok(nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                None,
            ).is_ok())
        } else {
            Ok(false)
        }
    }
    
    /// Tail logs
    pub async fn tail_logs(&self, options: LogOptions) -> Result<()> {
        if options.follow {
            self.tail_follow()?;
        } else {
            self.tail_static(options.lines)?;
        }
    }
}
```

### Server Binary Modes

**File**: `toygres-server/src/main.rs`

```rust
#[derive(Parser)]
#[command(name = "toygres-server")]
struct Args {
    #[command(subcommand)]
    mode: ServerMode,
}

#[derive(Subcommand)]
enum ServerMode {
    /// Run as standalone server (API + Workers) - for local dev
    Standalone {
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        #[arg(short, long, default_value = "1")]
        workers: usize,
    },
    
    /// Run as API server only (no workers) - for production
    Api {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    
    /// Run as worker only (no API) - for production
    Worker {
        #[arg(long)]
        worker_id: Option<String>,
    },
    
    /// Run a single CLI command (legacy, maintains current behavior)
    #[command(flatten)]
    Command(Commands),
}

#[derive(Subcommand)]
enum Commands {
    Create { /* ... */ },
    Delete { /* ... */ },
}
```

**Usage**:
```bash
# Local development (all-in-one)
toygres-server standalone --port 8080 --workers 2

# Production API pod
toygres-server api --port 8080

# Production worker pod
toygres-server worker --worker-id work-01

# Legacy CLI mode (current behavior)
toygres-server create mydb --password xxx
```

---

## CLI Server Management

### Commands

```bash
toygres server start [OPTIONS]     # Start local server
toygres server stop                # Stop local server
toygres server restart             # Restart local server
toygres server status              # Check if running
toygres server logs [OPTIONS]      # View/tail logs
toygres server config              # Show server config
```

### Auto-Start Behavior

When CLI needs the server but it's not running:

```bash
$ toygres list

⚠️  No toygres server found.

Options:
  1. Start local server:        toygres server start --background
  2. Connect to remote server:  toygres config set api-url <url>

Start local server now? (Y/n) █
```

**Auto-start sequence**:
```
1. Check if server configured in config (api-url)
   → Yes: Try connecting
   → No: Check local server

2. Check if local server running
   → Yes: Use it
   → No: Prompt to start

3. If user confirms:
   → Start server in background
   → Wait for ready (health check)
   → Proceed with command
```

### Configuration Priority

```
1. Command-line flags:  --api-url http://localhost:8080
2. Environment var:     TOYGRES_API_URL=http://...
3. Config file:         ~/.toygres/config.yaml
4. Default:             http://localhost:8080 (local server)
```

---

## Log Management

### Log Output Destinations

**Foreground Mode**:
```bash
toygres server start --foreground

# Logs go to stdout/stderr (can pipe)
toygres server start | tee server.log
toygres server start 2>&1 | grep ERROR
```

**Background Mode**:
```bash
toygres server start --background

# Logs go to file
# Default: ~/.toygres/logs/server.log

# Custom log file
toygres server start --background --log-file /var/log/toygres/server.log
```

### Log Viewing

```bash
# View recent logs
toygres server logs

# Follow logs (live tail)
toygres server logs --follow

# Last N lines
toygres server logs --tail 200

# Filter by level
toygres server logs --level error

# Search
toygres server logs --grep "orchestration.*failed"

# Time range
toygres server logs --since "1 hour ago"
toygres server logs --since "2024-11-14 12:00:00" --until "2024-11-14 13:00:00"

# JSON output (for parsing)
toygres server logs -o json | jq 'select(.level == "ERROR")'
```

**Log Output**:
```
2024-11-14 14:00:00.123 [INFO]  toygres_server: Server started
2024-11-14 14:00:00.456 [INFO]  duroxide::runtime: Runtime initialized
2024-11-14 14:00:01.789 [INFO]  toygres_server: API listening on 0.0.0.0:8080
2024-11-14 14:05:23.012 [INFO]  toygres_server: POST /api/instances
2024-11-14 14:05:23.345 [INFO]  duroxide: Started orchestration: create-proddb-a1b2c3d4
2024-11-14 14:05:24.678 [INFO]  duroxide::activity: deploy-postgres (proddb-a1b2c3d4)
2024-11-14 14:05:25.901 [INFO]  duroxide::activity: PVC created
2024-11-14 14:05:26.234 [INFO]  duroxide::activity: StatefulSet created
...
```

**Log Format (JSON)**:
```json
{
  "timestamp": "2024-11-14T14:00:00.123Z",
  "level": "INFO",
  "target": "toygres_server",
  "message": "Server started",
  "fields": {
    "port": 8080,
    "mode": "standalone"
  }
}
```

---

## CLI Structure

### Unified Binary Approach

**Build**: One binary that can act as CLI or server

```
toygres (symlinked to toygres-server)
├── When invoked as "toygres" → CLI mode
├── When invoked as "toygres-server" → Server mode
└── Flag --mode can override
```

**Binary Installation**:
```bash
# Install both
cargo install --path toygres-server

# Creates:
# - ~/.cargo/bin/toygres-server
# - ~/.cargo/bin/toygres (symlink)

# Or use the same binary for both
ln -s toygres-server toygres
```

---

## Deployment Scenarios

### Scenario 1: Local Development

```bash
# Start embedded server
toygres server start --background

# Use CLI
toygres create mydb --password test123
toygres list
toygres get mydb

# Stop when done
toygres server stop
```

**State Storage**:
```
~/.toygres/
├── config.yaml         # CLI configuration
├── server.pid          # Server PID
└── logs/
    └── server.log      # Server logs
```

---

### Scenario 2: Production on AKS

```yaml
# Deployment: API Server
apiVersion: apps/v1
kind: Deployment
metadata:
  name: toygres-api
  namespace: toygres-control-plane
spec:
  replicas: 3
  selector:
    matchLabels:
      app: toygres-api
  template:
    metadata:
      labels:
        app: toygres-api
    spec:
      containers:
      - name: api
        image: ghcr.io/affandar/toygres-server:latest
        args: ["api", "--port", "8080"]
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: toygres-secrets
              key: database-url
        - name: RUST_LOG
          value: "info"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 8080

---
# Deployment: Workers
apiVersion: apps/v1
kind: Deployment
metadata:
  name: toygres-workers
  namespace: toygres-control-plane
spec:
  replicas: 5
  selector:
    matchLabels:
      app: toygres-worker
  template:
    metadata:
      labels:
        app: toygres-worker
    spec:
      containers:
      - name: worker
        image: ghcr.io/affandar/toygres-server:latest
        args: ["worker"]
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: toygres-secrets
              key: database-url
        - name: RUST_LOG
          value: "info"

---
# Service: API
apiVersion: v1
kind: Service
metadata:
  name: toygres-api
  namespace: toygres-control-plane
spec:
  type: LoadBalancer
  selector:
    app: toygres-api
  ports:
  - port: 80
    targetPort: 8080
    name: http
```

**CLI Configuration (for production)**:
```bash
# Configure CLI to use production API
toygres config set api-url http://toygres-api.toygres-control-plane.svc.cluster.local

# Or from outside cluster
toygres config set api-url https://api.toygres.io
toygres config set api-token <your-token>

# Use CLI
toygres list
toygres create proddb --password xxx
```

---

### Scenario 3: CI/CD Pipeline

```bash
#!/bin/bash
# deploy.sh - CI/CD script

set -e

# Configure CLI (no server management)
export TOYGRES_API_URL=https://api.toygres.io
export TOYGRES_API_TOKEN="${CI_TOKEN}"

# Create database
INSTANCE_JSON=$(toygres create "${CI_COMMIT_SHA}" \
  --password "${DB_PASSWORD}" \
  --tag "branch=${CI_BRANCH}" \
  --tag "commit=${CI_COMMIT_SHA}" \
  --output json \
  --wait)

# Extract connection string
DB_HOST=$(echo "$INSTANCE_JSON" | jq -r '.dns_name')
export DATABASE_URL="postgresql://postgres:${DB_PASSWORD}@${DB_HOST}:5432/postgres"

# Run migrations
./run-migrations.sh

# Run tests
npm test

# Cleanup
toygres delete "${CI_COMMIT_SHA}" --force
```

---

## Project Structure

### Unified Crate: `toygres`

```
toygres/
├── Cargo.toml                    # Workspace root
├── toygres-models/               # Shared types
├── toygres-orchestrations/       # Duroxide orchestrations
├── toygres-server/               # Server binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Entry point (mode selection)
│       ├── server/
│       │   ├── mod.rs
│       │   ├── standalone.rs     # Standalone mode
│       │   ├── api.rs            # API mode
│       │   └── worker.rs         # Worker mode
│       ├── api/
│       │   ├── mod.rs
│       │   ├── routes/
│       │   │   ├── instances.rs  # Instance endpoints
│       │   │   ├── orchestrations.rs # Orchestration endpoints
│       │   │   └── health.rs     # Health endpoints
│       │   └── state.rs          # Shared state
│       └── cli/
│           ├── mod.rs
│           ├── commands/
│           │   ├── create.rs
│           │   ├── delete.rs
│           │   ├── list.rs
│           │   ├── get.rs
│           │   ├── health.rs
│           │   ├── connect.rs
│           │   ├── test.rs
│           │   ├── orchestrations.rs
│           │   ├── stats.rs
│           │   ├── server.rs      # Server management
│           │   └── config.rs
│           ├── output/
│           │   ├── table.rs
│           │   ├── json.rs
│           │   └── yaml.rs
│           └── client.rs          # API client
└── toygres-web/                  # Future: Web UI
    └── ...
```

**Binary Names**:
```bash
# Two binaries from same crate:
[[bin]]
name = "toygres-server"    # Server binary
path = "src/main.rs"

[[bin]]
name = "toygres"           # CLI binary (same source, different behavior)
path = "src/main.rs"
```

**Entry Point Logic**:
```rust
// toygres-server/src/main.rs
fn main() -> Result<()> {
    let binary_name = std::env::args().next()
        .and_then(|path| Path::new(&path).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("toygres-server");
    
    match binary_name {
        "toygres" => cli_main(),           // CLI commands
        "toygres-server" => server_main(), // Server modes
        _ => auto_detect_mode(),           // Detect from args
    }
}
```

---

## Alternative: Separate Binaries

### Option B: Two Separate Crates

```
toygres/
├── toygres-server/      # Server binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs      # Server modes (standalone, api, worker)
│       ├── api/
│       └── worker/
│
├── toygres-cli/         # CLI binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs      # CLI entry point
│       ├── commands/    # All CLI commands
│       ├── client.rs    # HTTP client
│       └── server.rs    # Server management (starts toygres-server)
```

**Pros**:
- Clean separation of concerns
- CLI doesn't include server dependencies
- Smaller CLI binary size

**Cons**:
- Two binaries to install
- More complex build/release process
- CLI needs to find/execute toygres-server binary

---

## Recommended Approach

### **Hybrid: Single Crate, Mode Detection**

**Why?**
1. ✅ **Simpler distribution** - One binary to install
2. ✅ **Development friendly** - Easy local testing
3. ✅ **Production flexible** - Can run in split mode on AKS
4. ✅ **Backward compatible** - Current CLI commands still work

**Implementation**:

```rust
// Detect mode from invocation
match detect_mode() {
    Mode::CLI => run_cli(),        // User ran: toygres create/list/etc
    Mode::ServerStandalone => run_server_standalone(),  // toygres-server standalone
    Mode::ServerAPI => run_server_api(),                // toygres-server api
    Mode::ServerWorker => run_server_worker(),          // toygres-server worker
}
```

**CLI Commands**:
```bash
# These invoke CLI mode (quick, one-off commands)
toygres create mydb --password xxx       # Starts orch via API, exits
toygres list                             # Queries API, prints, exits
toygres get mydb                         # Queries API, prints, exits

# These manage local server (for development)
toygres server start --background        # Starts server process, exits
toygres server logs --follow             # Tails logs, blocks
toygres server stop                      # Stops server, exits

# This is the server (long-running)
toygres-server standalone                # Runs server until killed
toygres-server api                       # Runs API server until killed
toygres-server worker                    # Runs worker until killed
```

---

## Configuration Files

### CLI Config: `~/.toygres/config.yaml`

```yaml
# API Configuration
api:
  url: http://localhost:8080      # API endpoint
  token: ""                        # Auth token (future)
  timeout: 30s                     # Request timeout

# Default Values
defaults:
  namespace: toygres
  output_format: table
  postgres_version: "18"
  storage_size_gb: 10

# Server Management (local dev)
server:
  auto_start: true                 # Auto-start if not running
  pid_file: ~/.toygres/server.pid
  log_file: ~/.toygres/logs/server.log
  default_port: 8080
  default_workers: 1

# Display Options
display:
  colors: true
  timestamps: relative             # relative or absolute
  timezone: local
  table_style: unicode             # unicode, ascii, or plain
```

### Server Config: Environment Variables

Server configured via environment variables (12-factor app):

```bash
# Required
DATABASE_URL=postgresql://user:pass@host:5432/db
AKS_CLUSTER_NAME=toygres-aks
AKS_RESOURCE_GROUP=toygres-rg

# Optional
AKS_NAMESPACE=toygres
RUST_LOG=info
API_PORT=8080
WORKER_THREADS=1
WORKER_ID=work-01
```

---

## Development Workflow

### Developer Experience

```bash
# First time setup
git clone https://github.com/affandar/toygres
cd toygres
cp .env.example .env
# Edit .env with your DATABASE_URL, AKS config

./scripts/db-init.sh

# Build
cargo build --release

# Start server for development (auto-background)
toygres server start --background

# Use CLI
toygres create testdb --password test123
toygres list
toygres get testdb --show-health

# View server logs
toygres server logs --follow

# Stop server
toygres server stop
```

### Testing Workflow

```bash
# Unit tests
cargo test

# Integration tests (requires running server)
toygres server start --background
cargo test --test integration_tests
toygres server stop

# E2E tests (requires AKS access)
./scripts/run-e2e-tests.sh
```

---

## Questions for Review

1. **Binary Distribution**: Single binary or separate CLI/server?
2. **Auto-Start**: Should CLI auto-start server, or require explicit start?
3. **Log Management**: File-based logs vs systemd/journald integration?
4. **Process Management**: Use OS process manager or custom solution?
5. **Server Status File**: PID file sufficient or need richer status file?
6. **Graceful Shutdown**: Timeout for graceful stop before force kill?

---

## Implementation Phases

### Phase 1: Server Modes (Current → Refactor)
- ✅ Split current CLI into server modes
- ✅ Add `standalone`, `api`, `worker` modes
- ✅ Keep backward compatibility

### Phase 2: CLI Server Management
- ✅ Implement `toygres server start/stop/status/logs`
- ✅ Add process management (PID file, signals)
- ✅ Add log file rotation
- ✅ Add auto-start detection

### Phase 3: CLI Commands (via API)
- ✅ Convert current CLI to use API client
- ✅ Add rich output formatting
- ✅ Add orchestration introspection commands

### Phase 4: Polish
- ✅ Shell completion
- ✅ Interactive prompts
- ✅ Progress indicators
- ✅ Error handling

---

## Success Criteria

- ✅ Single binary works as both CLI and server
- ✅ CLI can manage local server for development
- ✅ Server runs split (API + workers) in production
- ✅ CLI can connect to remote servers
- ✅ Logs are accessible and searchable
- ✅ Process management is reliable
- ✅ Works seamlessly in local dev and production
- ✅ CI/CD friendly (no server management needed)

