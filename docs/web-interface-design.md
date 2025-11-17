# Toygres Web Interface Design

## Overview

Design a modern web interface for Toygres - a managed PostgreSQL service. The web UI enables users to create, manage, and monitor PostgreSQL instances running on Azure Kubernetes Service (AKS).

---

## Architecture Overview

### Deployment Topology on AKS

```
┌─────────────────────────────────────────────────────────────┐
│                     Azure Kubernetes Service                 │
│                                                              │
│  ┌────────────────┐  ┌────────────────┐  ┌───────────────┐ │
│  │  Web Frontend  │  │ Toygres Server │  │  Data Plane   │ │
│  │  (Deployment)  │  │  (Deployment)  │  │ (StatefulSets)│ │
│  │                │  │                │  │               │ │
│  │  - React/Next  │  │  - Rust API    │  │  - PostgreSQL │ │
│  │  - Static Site │  │  - Workers     │  │    Instances  │ │
│  │  - Public LB   │  │  - Duroxide    │  │  - User DBs   │ │
│  └────────┬───────┘  └────────┬───────┘  └───────────────┘ │
│           │                   │                             │
│           └───────────────────┘                             │
│                      │                                      │
│                      ▼                                      │
│           ┌──────────────────────┐                         │
│           │   Control Metadata   │                         │
│           │    PostgreSQL DB     │                         │
│           │  (External/Managed)  │                         │
│           │                      │                         │
│           │  - toygres_cms       │                         │
│           │  - toygres_duroxide  │                         │
│           └──────────────────────┘                         │
└─────────────────────────────────────────────────────────────┘
```

### Kubernetes Resource Terminology

In AKS/Kubernetes:

1. **Deployment** - For stateless applications (web UI, API workers)
   - Multiple replicas for high availability
   - Rolling updates
   - Automatic scaling

2. **StatefulSet** - For stateful applications (PostgreSQL instances)
   - Stable network identities
   - Persistent storage
   - Ordered deployment/scaling

3. **Service** - Networking/load balancing
   - LoadBalancer (public IPs)
   - ClusterIP (internal)
   - NodePort (port mapping)

4. **Ingress** - HTTP/HTTPS routing
   - Single entry point
   - SSL/TLS termination
   - Path-based routing

### Toygres Deployment Model

```yaml
# Web Frontend
Deployment: toygres-web
  - Replicas: 2-3
  - Container: nginx serving static React app
  - Service: LoadBalancer (public HTTPS)
  - Ingress: app.toygres.io

# Control Plane Workers
Deployment: toygres-workers
  - Replicas: 2-5 (for duroxide workers)
  - Container: toygres-server (Rust binary)
  - Service: None (internal processing only)

# Data Plane (User PostgreSQL Instances)
StatefulSet: {user-db-name}-{guid}
  - Replicas: 1 per instance
  - PersistentVolume: Azure Disk
  - Service: LoadBalancer (per instance)
```

---

## Technology Stack

### Frontend

**Framework**: Next.js 14+ (React)
- Server-side rendering for SEO
- File-based routing
- API routes (optional proxy)
- TypeScript for type safety

**UI Library**: shadcn/ui + Tailwind CSS
- Modern, accessible components
- Customizable design system
- Dark mode support
- Responsive by default

**State Management**: 
- TanStack Query (React Query) for server state
- Zustand for client state (if needed)

**API Client**: 
- Fetch API with TypeScript types
- Auto-generated from OpenAPI spec (future)

**Build/Deploy**:
- Docker container with nginx
- Static export for CDN deployment (optional)

### Backend API

**Framework**: Axum (already in use)
- RESTful endpoints
- JSON responses
- CORS enabled for web UI
- JWT/session authentication (future)

**Existing Structure**:
```
toygres-server/src/
  ├── main.rs      # CLI (can extract API mode)
  ├── api.rs       # Future: REST endpoints
  └── worker.rs    # Duroxide worker mode
```

---

## Pages & Features

### 1. Dashboard (Home)

**Route**: `/`

**Purpose**: Overview of all instances and system health

**Layout**:
```
┌─────────────────────────────────────────────────────┐
│  Toygres Logo        Dashboard    Instances   Docs  │
├─────────────────────────────────────────────────────┤
│                                                      │
│  📊 Overview                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ Running  │  │ Creating │  │ Unhealthy│          │
│  │    12    │  │     2    │  │     1    │          │
│  └──────────┘  └──────────┘  └──────────┘          │
│                                                      │
│  📋 Recent Instances                                 │
│  ┌────────────────────────────────────────────────┐ │
│  │ Name       State    Health   Created    Actions│ │
│  ├────────────────────────────────────────────────┤ │
│  │ proddb     Running  ●        2h ago     [...]  │ │
│  │ testdb1    Creating -        5m ago     [...]  │ │
│  │ analytics  Running  ●        1d ago     [...]  │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  📈 System Health (Future)                           │
│  [Chart showing instance health over time]          │
│                                                      │
└─────────────────────────────────────────────────────┘
```

**Data Sources**:
- GET `/api/instances` - List all instances
- GET `/api/stats` - System statistics
- WebSocket (future): Real-time updates

**Key Metrics**:
- Total instances by state (running, creating, deleting, failed)
- Health status distribution
- Recent deployments
- System capacity (future)

---

### 2. Instances List

**Route**: `/instances`

**Purpose**: View and manage all PostgreSQL instances

**Layout**:
```
┌─────────────────────────────────────────────────────┐
│  Toygres                                             │
├─────────────────────────────────────────────────────┤
│                                                      │
│  PostgreSQL Instances                 [+ New Instance]│
│                                                      │
│  Filters: [All States ▾] [Health ▾] [Search...]    │
│                                                      │
│  ┌────────────────────────────────────────────────┐ │
│  │ Name         DNS Name        State    Health   │ │
│  ├────────────────────────────────────────────────┤ │
│  │ ● proddb     proddb.west... Running  Healthy  │ │
│  │   PostgreSQL 18 • 10GB • Created 2h ago       │ │
│  │   [Connect] [Details] [Delete]                 │ │
│  │                                                 │ │
│  │ ⏳ testdb1   testdb1.wes... Creating  Unknown │ │
│  │   PostgreSQL 18 • 10GB • Started 5m ago       │ │
│  │   [Cancel] [Details]                           │ │
│  │                                                 │ │
│  │ ⚠️ olddb     olddb.west... Running  Unhealthy │ │
│  │   PostgreSQL 16 • 20GB • Created 30d ago      │ │
│  │   Last check failed: Connection timeout        │ │
│  │   [Investigate] [Details] [Delete]             │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  Showing 3 of 15 instances      [1] 2 3 4 › »      │
│                                                      │
└─────────────────────────────────────────────────────┘
```

**Features**:
- **Filtering**: By state, health, version, age
- **Sorting**: By name, created date, health
- **Search**: Full-text search on name, DNS name
- **Bulk Actions**: Delete multiple instances (future)
- **Real-time Updates**: Health status changes live

**API Endpoints**:
```
GET  /api/instances
  Query params:
    - state: creating|running|deleting|deleted|failed
    - health: healthy|unhealthy|unknown
    - page: number
    - limit: number
    - search: string

Response:
{
  "instances": [
    {
      "id": "uuid",
      "user_name": "proddb",
      "k8s_name": "proddb-a1b2c3d4",
      "dns_name": "proddb",
      "state": "running",
      "health_status": "healthy",
      "postgres_version": "18",
      "storage_size_gb": 10,
      "ip_connection_string": "postgresql://...",
      "dns_connection_string": "postgresql://...",
      "created_at": "2024-11-14T12:00:00Z",
      "updated_at": "2024-11-14T14:00:00Z"
    }
  ],
  "total": 15,
  "page": 1,
  "pages": 2
}
```

---

### 3. Create Instance

**Route**: `/instances/new`

**Purpose**: Wizard for creating a new PostgreSQL instance

**Layout**:
```
┌─────────────────────────────────────────────────────┐
│  Toygres    < Back to Instances                      │
├─────────────────────────────────────────────────────┤
│                                                      │
│  Create PostgreSQL Instance                          │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                      │
│  Basic Configuration                                 │
│  ┌────────────────────────────────────────────────┐ │
│  │ Database Name *                                 │ │
│  │ [proddb_____________]                           │ │
│  │ This will be your DNS name:                     │ │
│  │ proddb.westus3.cloudapp.azure.com              │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  ┌────────────────────────────────────────────────┐ │
│  │ Password *                                      │ │
│  │ [••••••••••••••] [Generate]                    │ │
│  │ ✓ Strong password (12+ chars, mixed case)      │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  PostgreSQL Configuration                            │
│  ┌────────────────────────────────────────────────┐ │
│  │ Version                                         │ │
│  │ ● PostgreSQL 18 (latest)                       │ │
│  │ ○ PostgreSQL 16                                │ │
│  │ ○ PostgreSQL 15                                │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  ┌────────────────────────────────────────────────┐ │
│  │ Storage Size                                    │ │
│  │ [10___________] GB (10-1000 GB)                │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  Networking                                          │
│  ┌────────────────────────────────────────────────┐ │
│  │ ☑ Public IP (LoadBalancer)                     │ │
│  │ ☐ Internal only (ClusterIP)                    │ │
│  │                                                 │ │
│  │ ℹ️ Public IP recommended for external access   │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  Advanced Options (Optional)                         │
│  ┌────────────────────────────────────────────────┐ │
│  │ Namespace:  [toygres_______]                   │ │
│  │ Tags:       [+ Add tag]                         │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  Estimated Cost: $XX/month (future)                 │
│                                                      │
│             [Cancel]  [Create Instance]              │
│                                                      │
└─────────────────────────────────────────────────────┘
```

**Validation**:
- Name: 3-63 chars, lowercase, alphanumeric + hyphens
- DNS uniqueness check (API call)
- Password strength requirements
- Storage size limits

**API Endpoint**:
```
POST /api/instances
Body:
{
  "name": "proddb",
  "password": "SecurePass123!",
  "postgres_version": "18",
  "storage_size_gb": 10,
  "use_load_balancer": true,
  "namespace": "toygres",
  "tags": {
    "environment": "production",
    "team": "backend"
  }
}

Response (202 Accepted):
{
  "instance_id": "uuid",
  "k8s_name": "proddb-a1b2c3d4",
  "dns_name": "proddb",
  "orchestration_id": "create-proddb-a1b2c3d4",
  "state": "creating",
  "message": "Instance creation started"
}
```

**Flow**:
1. User fills form
2. Client validates inputs
3. POST to `/api/instances`
4. Show loading modal with progress
5. Redirect to instance details page
6. Poll for status updates

---

### 4. Instance Details

**Route**: `/instances/:name`

**Purpose**: Detailed view and management of a single instance

**Layout**:
```
┌─────────────────────────────────────────────────────┐
│  Toygres    < Back to Instances                      │
├─────────────────────────────────────────────────────┤
│                                                      │
│  proddb                                ● Healthy     │
│  PostgreSQL 18 • Running • Created 2 hours ago       │
│                                                      │
│  ┌─ Overview ─┬─ Health ─┬─ Connection ─┬─ Logs ─┐ │
│                                                      │
│  📊 Overview                                         │
│  ┌────────────────────────────────────────────────┐ │
│  │ Status:           Running                       │ │
│  │ Health:           ● Healthy (last check: 30s)  │ │
│  │ K8s Name:         proddb-a1b2c3d4              │ │
│  │ DNS Name:         proddb.westus3.cloudapp...   │ │
│  │ PostgreSQL:       18.0                          │ │
│  │ Storage:          10 GB                         │ │
│  │ Networking:       LoadBalancer (Public)         │ │
│  │ Created:          Nov 14, 2024 2:00 PM         │ │
│  │ Namespace:        toygres                       │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  🔗 Connection Strings                               │
│  ┌────────────────────────────────────────────────┐ │
│  │ DNS (Recommended)                               │ │
│  │ postgresql://postgres:***@proddb.westus3...    │ │
│  │                                    [Copy] [Show]│ │
│  │                                                 │ │
│  │ IP Address                                      │ │
│  │ postgresql://postgres:***@4.249.117.85:5432    │ │
│  │                                    [Copy] [Show]│ │
│  │                                                 │ │
│  │ Azure DNS                                       │ │
│  │ proddb.westus3.cloudapp.azure.com:5432         │ │
│  │                                          [Copy] │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  ⚙️ Actions                                          │
│  [🔄 Test Connection] [📊 View Metrics] [🗑️ Delete] │
│                                                      │
└─────────────────────────────────────────────────────┘
```

**Tabs**:

1. **Overview** (shown above)
   - Instance metadata
   - Connection strings
   - Quick actions

2. **Health**
   ```
   Health Check History (Last 24 Hours)
   
   [Chart showing health over time]
   
   Recent Checks:
   ✓ 30 seconds ago  - Healthy (response: 142ms)
   ✓ 1 minute ago    - Healthy (response: 138ms)
   ✓ 1.5 minutes ago - Healthy (response: 145ms)
   ...
   ```

3. **Connection**
   - Connection examples (psql, Node.js, Python, etc.)
   - SSL/TLS configuration
   - Firewall rules (future)

4. **Logs** (future)
   - PostgreSQL logs
   - K8s pod logs
   - Orchestration history

**API Endpoints**:
```
GET /api/instances/:name
Response:
{
  "id": "uuid",
  "user_name": "proddb",
  "k8s_name": "proddb-a1b2c3d4",
  "dns_name": "proddb",
  "state": "running",
  "health_status": "healthy",
  "postgres_version": "18",
  "storage_size_gb": 10,
  "use_load_balancer": true,
  "ip_connection_string": "postgresql://...",
  "dns_connection_string": "postgresql://...",
  "external_ip": "4.249.117.85",
  "created_at": "2024-11-14T12:00:00Z",
  "updated_at": "2024-11-14T14:00:00Z",
  "health_check_orchestration_id": "health-proddb-a1b2c3d4"
}

GET /api/instances/:name/health
Response:
{
  "current_status": "healthy",
  "last_check": "2024-11-14T14:00:30Z",
  "history": [
    {
      "status": "healthy",
      "checked_at": "2024-11-14T14:00:30Z",
      "response_time_ms": 142,
      "postgres_version": "PostgreSQL 18.0"
    },
    ...
  ]
}

DELETE /api/instances/:name
Response (202 Accepted):
{
  "orchestration_id": "delete-proddb-a1b2c3d4",
  "message": "Instance deletion started"
}
```

---

## Visual Design

### Color Palette

**Primary (PostgreSQL Blue)**:
- `#336791` - PostgreSQL brand color
- `#447EB2` - Lighter variant
- `#224466` - Darker variant

**Status Colors**:
- Success/Healthy: `#22c55e` (green-500)
- Warning: `#f59e0b` (amber-500)
- Error/Unhealthy: `#ef4444` (red-500)
- Info/Creating: `#3b82f6` (blue-500)

**Neutral**:
- Background: `#ffffff` (light) / `#0f172a` (dark)
- Surface: `#f8fafc` (light) / `#1e293b` (dark)
- Text: `#0f172a` (light) / `#f1f5f9` (dark)

### Typography

- **Font**: Inter (system fonts fallback)
- **Headings**: Bold, 24-32px
- **Body**: Regular, 14-16px
- **Code/Mono**: JetBrains Mono, 14px

### Components

**State Indicators**:
```
● Healthy (green)
⏳ Creating (blue, animated)
⚠️ Unhealthy (red)
🔄 Deleting (gray, animated)
❌ Failed (red)
```

**Cards**: Subtle borders, slight shadow, rounded corners
**Buttons**: Primary (blue), Secondary (gray), Destructive (red)
**Forms**: Clear labels, inline validation, helpful hints

---

## API Server Updates

### New API Mode

**File**: `toygres-server/src/main.rs`

Add API server mode alongside CLI:

```rust
enum ServerMode {
    CLI(Commands),    // Existing CLI commands
    API { port: u16 }, // New: REST API server
    Worker,            // New: Duroxide worker only
}
```

### REST Endpoints

**File**: `toygres-server/src/api/mod.rs` (new)

```rust
pub mod instances;
pub mod health;
pub mod stats;

pub fn create_router() -> Router {
    Router::new()
        .route("/api/instances", get(list_instances).post(create_instance))
        .route("/api/instances/:name", get(get_instance).delete(delete_instance))
        .route("/api/instances/:name/health", get(get_health_history))
        .route("/api/stats", get(get_stats))
        .route("/health", get(health_check))
        .layer(CorsLayer::permissive()) // Configure for production
}
```

### Deployment Configuration

**Split Concerns**:

1. **API Server Pod**
   ```bash
   toygres-server api --port 8080
   ```
   - Handles HTTP requests
   - Starts orchestrations via Duroxide client
   - Reads from CMS database

2. **Worker Pod**
   ```bash
   toygres-server worker
   ```
   - Runs Duroxide runtime
   - Executes activities
   - Processes orchestrations

Benefits:
- Scale API and workers independently
- Workers don't need public exposure
- API can be stateless (easier horizontal scaling)

---

## Deployment Architecture

### Docker Images

**1. Web Frontend**
```dockerfile
# Dockerfile.web
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/out /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80
```

**2. Toygres Server (API + Worker)**
```dockerfile
# Dockerfile.server
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin toygres-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/toygres-server /usr/local/bin/
EXPOSE 8080
CMD ["toygres-server"]
```

### Kubernetes Manifests

**Directory Structure**:
```
k8s/
├── web/
│   ├── deployment.yaml
│   ├── service.yaml
│   └── ingress.yaml
├── server/
│   ├── api-deployment.yaml
│   ├── api-service.yaml
│   ├── worker-deployment.yaml
│   └── configmap.yaml
└── postgres/ (data plane instances)
    └── (created dynamically by toygres)
```

---

## Development Phases

### Phase 1: API Foundation
1. ✅ Split toygres-server into API and Worker modes
2. ✅ Implement REST endpoints for instances
3. ✅ Add CORS configuration
4. ✅ Create OpenAPI spec
5. ✅ Test API with Postman/curl

### Phase 2: Web UI Skeleton
1. ✅ Set up Next.js project
2. ✅ Configure shadcn/ui + Tailwind
3. ✅ Create layout and navigation
4. ✅ Implement dashboard (static)
5. ✅ Implement instances list (static)

### Phase 3: API Integration
1. ✅ Connect dashboard to API
2. ✅ Implement instances list with real data
3. ✅ Add create instance form
4. ✅ Add instance details page
5. ✅ Real-time status updates (polling)

### Phase 4: Polish & Deploy
1. ✅ Error handling and loading states
2. ✅ Dark mode
3. ✅ Responsive design
4. ✅ Docker images
5. ✅ Kubernetes manifests
6. ✅ Deploy to AKS

---

## Future Enhancements

### Authentication & Authorization
- User accounts (Azure AD integration)
- API keys
- Role-based access control
- Multi-tenancy

### Advanced Features
- **Backups**: Schedule and restore
- **Metrics**: CPU, memory, disk, connections
- **Alerts**: Email/Slack notifications
- **Scaling**: Resize storage, change versions
- **Migrations**: Import existing databases
- **Extensions**: Install PostgreSQL extensions

### Observability
- Grafana dashboards
- Prometheus metrics
- Distributed tracing
- Audit logs

---

## Questions for Review

1. **Deployment Model**: Should API and workers be in the same pod or separate deployments?
2. **Authentication**: When to add auth (now vs later)?
3. **Pricing**: Should we show cost estimates in the UI?
4. **Regions**: Multi-region support from the start?
5. **Customization**: How much PostgreSQL configuration to expose?
6. **Backups**: Priority for Phase 1 or defer?

---

## Success Criteria

- ✅ Users can create PostgreSQL instances via web UI
- ✅ Clear visibility into instance status and health
- ✅ Easy access to connection strings
- ✅ Intuitive, modern design
- ✅ Fast loading times (< 2s page loads)
- ✅ Mobile responsive
- ✅ Production-ready deployment on AKS

