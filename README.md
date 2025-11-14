# Toygres

A Rust-based control plane for hosting PostgreSQL containers as a service on Azure Kubernetes Service (AKS).

## Features

- **Durable Workflows**: Uses [Duroxide](https://github.com/affandar/duroxide) for reliable orchestration
- **Kubernetes Native**: Deploys PostgreSQL as pods in AKS
- **Public & Private Access**: Supports LoadBalancer (public IP) or ClusterIP (internal only)
- **DNS Support**: Automatic Azure DNS names for instances
- **YAML Templates**: Kubernetes resources defined in clean, readable YAML
- **REST API**: Simple API for instance management (coming soon)
- **PostgreSQL Metadata**: Stores deployment metadata in PostgreSQL (coming soon)

## Architecture

- **Deployment**: PostgreSQL containers as StatefulSets in AKS
- **Storage**: Persistent volumes for durable data
- **Networking**: LoadBalancer services with Azure DNS names
- **Workflow Engine**: Duroxide for durable orchestrations
- **Control Plane**: Rust-based API server

## Quick Start

### Prerequisites

1. **Rust** (1.85.0 or newer)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Docker Desktop** (for kind local testing)
   - Download from: https://www.docker.com/products/docker-desktop
   - Make sure Docker is running

3. **Azure CLI**
   ```bash
   brew install azure-cli
   az login
   ```

4. **kubectl**
   ```bash
   brew install kubectl
   ```

5. **kind** (optional, for local testing)
   ```bash
   brew install kind
   ```

### Azure Infrastructure Setup

#### Option 1: Use Existing AKS Cluster

If you already have an AKS cluster:

```bash
# Get cluster credentials
az aks get-credentials --resource-group <your-rg> --name <your-cluster>

# Verify connection
kubectl cluster-info

# Create toygres namespace
kubectl create namespace toygres

# Update .env with your cluster details
cp .env.example .env
# Edit .env and set:
#   AKS_CLUSTER_NAME=<your-cluster>
#   AKS_RESOURCE_GROUP=<your-rg>
#   AKS_NAMESPACE=toygres
```

#### Option 2: Create New AKS Cluster

Use the provided infrastructure setup script:

```bash
# This will create:
# - Azure Resource Group
# - AKS Cluster (takes 10-15 minutes)
# - toygres namespace
# - Storage class configuration
./scripts/setup-infra.sh

# Script will prompt for configuration and output values for .env
```

### Configuration

1. **Copy the example environment file:**
   ```bash
   cp .env.example .env
   ```

2. **Edit `.env` with your values:**

   **Required for Control Plane:**
   ```bash
   DATABASE_URL=postgresql://user:password@host:5432/toygres
   AKS_CLUSTER_NAME=your-aks-cluster
   AKS_RESOURCE_GROUP=your-resource-group
   ```

   **Optional (for DNS support):**
   ```bash
   DNS_LABEL=toygres
   ```

   **Optional (for examples/testing):**
   ```bash
   INSTANCE_NAME=my-test-pg
   POSTGRES_PASSWORD=your-secure-password
   USE_LOAD_BALANCER=true
   ```

3. **Set up metadata database** (when ready to use control plane):
   ```bash
   ./scripts/db-init.sh
   ./scripts/db-migrate.sh   # (no-op until we add 0002+ migrations)
   ```

### Test Deployment

Test that everything works by deploying a PostgreSQL instance:

```bash
# Deploy with auto-generated DNS name
cargo run --example manual_deploy -- --dns-name mytest --clean

# Or deploy with defaults
cargo run --example manual_deploy

# Expected output:
# ✓ PostgreSQL deployed to AKS
# ✓ External IP: 4.249.xxx.xxx
# ✓ DNS name: mytest-toygres.westus3.cloudapp.azure.com
# ✓ Connection verified
```

### Build and Run Control Plane

```bash
# Build all crates
cargo build --workspace

# Run the control plane server (coming soon)
cargo run --bin toygres-server
```

## Project Structure

```
toygres/
├── toygres-models/          # Shared data structures
├── toygres-orchestrations/  # Duroxide orchestrations & activities
│   ├── src/
│   │   ├── activities/      # Atomic K8s operations
│   │   ├── orchestrations/  # Durable workflows
│   │   └── templates/       # Kubernetes YAML templates
├── toygres-server/          # Control plane server
│   ├── src/                 # Main server code
│   └── examples/            # Working examples (manual_deploy.rs)
├── docs/                    # Documentation
├── scripts/                 # Setup and management scripts
└── prompts/                 # AI assistant context docs
```

## Scripts

### Infrastructure Management

```bash
# Setup AKS cluster
./scripts/setup-infra.sh

# Setup metadata database schema + future migrations
./scripts/db-init.sh
./scripts/db-migrate.sh
```

### Deployment Management

```bash
# List all PostgreSQL deployments
./scripts/list-deployments.sh

# Clean up all deployments
./scripts/cleanup-deployments.sh

# Clean up a specific deployment
./scripts/cleanup-single.sh <instance-name>
```

## Examples

### Deploy PostgreSQL Instance

```bash
# With custom DNS name and auto-cleanup
cargo run --example manual_deploy -- --dns-name mydb --clean

# Keep instance running for testing
cargo run --example manual_deploy -- --dns-name prod-db

# Deploy without public DNS (IP only)
# Remove DNS_LABEL from .env, then:
cargo run --example manual_deploy
```

### Connect to Deployed Instance

The deployment tool outputs connection strings:

```bash
# Via DNS (recommended)
psql 'postgresql://postgres:password@mydb-toygres.westus3.cloudapp.azure.com:5432/postgres'

# Via IP
psql 'postgresql://postgres:password@4.249.xxx.xxx:5432/postgres'
```

### Clean Up Resources

```bash
# List current deployments
AKS_NAMESPACE=toygres ./scripts/list-deployments.sh

# Clean up a specific instance
AKS_NAMESPACE=toygres ./scripts/cleanup-single.sh mydb

# Or clean up all instances
AKS_NAMESPACE=toygres ./scripts/cleanup-deployments.sh
```

## Documentation

- **[docs/plan.md](docs/plan.md)** - Detailed implementation plan with phases
- **[docs/getting-started.md](docs/getting-started.md)** - Development guide
- **[docs/phase0-complete.md](docs/phase0-complete.md)** - Phase 0 summary
- **[docs/phase1-activities-plan.md](docs/phase1-activities-plan.md)** - Activities implementation plan
- **[prompts/project-context.md](prompts/project-context.md)** - AI assistant context

## API Endpoints (Coming in Phase 3)

- `POST /instances` - Create a new PostgreSQL instance
- `DELETE /instances/{id}` - Delete an instance
- `GET /instances` - List all instances
- `GET /instances/{id}` - Get instance details
- `GET /operations/{id}` - Monitor operation status
- `GET /health` - Control plane health check

## Development Status

### ✅ Phase 0: Complete
- Proof of concept working
- YAML-based K8s templates
- LoadBalancer with public IPs
- Azure DNS name support
- Connection testing
- Cleanup scripts

### 🔄 Phase 1: In Progress
- Extracting into Duroxide activities
- Following cross-crate registry pattern

### ⏳ Coming Soon
- Phase 2: Metadata database tracking
- Phase 3: REST API
- Phase 4: Duroxide orchestrations
- Phase 5: Health monitoring

## Troubleshooting

### Can't connect to AKS

```bash
# Get credentials
az aks get-credentials --resource-group <rg> --name <cluster> --overwrite-existing

# Verify
kubectl cluster-info
```

### Deployment fails

```bash
# Check namespace exists
kubectl get namespace toygres

# Create if missing
kubectl create namespace toygres

# Check storage classes
kubectl get storageclass
```

### Resources stuck

```bash
# Force delete
kubectl delete statefulset,svc,pvc -n toygres -l app=postgres --grace-period=0
```

## Contributing

See [docs/plan.md](docs/plan.md) for the implementation roadmap.

## License

MIT

