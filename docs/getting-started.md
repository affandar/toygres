# Getting Started with Toygres

## Project Structure

The Toygres project is organized as a Cargo workspace with the following structure:

```
toygres/
├── Cargo.toml                    # Workspace configuration
├── README.md                     # Project overview
├── .env.example                  # Environment variables template
├── .gitignore                    # Git ignore rules
├── docs/
│   ├── plan.md                   # Detailed implementation plan
│   └── getting-started.md        # This file
├── scripts/
│   ├── setup-infra.sh           # Azure infrastructure setup script
│   └── setup-db.sh              # Database schema setup script
├── toygres-models/              # Shared data structures
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs               # Data models: InstanceMetadata, DeploymentConfig, etc.
├── toygres-activities/          # Duroxide activities (atomic operations)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs               # Activity exports
│       ├── deploy.rs            # DeployPostgresActivity
│       ├── delete.rs            # DeletePostgresActivity
│       ├── status.rs            # GetInstanceStatusActivity
│       ├── health_check.rs      # HealthCheckActivity
│       ├── metadata.rs          # UpdateMetadataActivity
│       └── connection_string.rs # GenerateConnectionStringActivity
├── toygres-orchestrations/      # Duroxide orchestrations (workflows)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs               # Orchestration exports
│       ├── create_instance.rs   # CreateInstanceOrchestration
│       ├── delete_instance.rs   # DeleteInstanceOrchestration
│       ├── health_check.rs      # HealthCheckOrchestration
│       └── monitor_operation.rs # MonitorOperationOrchestration
└── toygres-server/              # Control plane server
    ├── Cargo.toml
    └── src/
        ├── main.rs              # Server entry point
        ├── config.rs            # Configuration loading
        ├── api.rs               # REST API endpoints
        └── worker.rs            # Duroxide worker setup
```

## Quick Start

### 1. Prerequisites

- **Rust**: Version 1.85.0 or newer
- **Azure CLI**: For infrastructure setup
- **PostgreSQL**: For metadata storage
- **Azure Account**: With access to create AKS resources

### 2. Initial Setup

```bash
# Clone the repository
cd toygres

# Copy environment configuration
cp .env.example .env

# Edit .env with your configuration
# Required variables:
#   - DATABASE_URL: PostgreSQL connection string
#   - AKS_CLUSTER_NAME: Your AKS cluster name
#   - AKS_RESOURCE_GROUP: Azure resource group
#   - AKS_NAMESPACE: Kubernetes namespace (default: toygres)
```

### 3. Infrastructure Setup (First Time Only)

If you don't have an AKS cluster yet, run the infrastructure setup script:

```bash
# This will create:
#   - Azure Resource Group
#   - AKS Cluster
#   - Kubernetes namespace for Toygres
#   - Storage class for PostgreSQL
./scripts/setup-infra.sh
```

The script will prompt you for configuration values and output the settings to add to your `.env` file.

### 4. Database Setup

Set up the metadata database schema:

```bash
# This will create:
#   - Custom types (instance_state, health_status)
#   - instances table
#   - Indexes and triggers
./scripts/setup-db.sh
```

### 5. Build and Run

```bash
# Build the project
cargo build

# Run the control plane server
cargo run --bin toygres-server
```

## Development Workflow

### Building Individual Crates

```bash
# Build only the models crate
cargo build -p toygres-models

# Build only the activities crate
cargo build -p toygres-activities

# Build only the orchestrations crate
cargo build -p toygres-orchestrations

# Build only the server
cargo build -p toygres-server
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p toygres-models
cargo test -p toygres-activities
```

### Checking for Issues

```bash
# Check all crates compile
cargo check

# Run the linter
cargo clippy

# Format code
cargo fmt
```

## Configuration

The control plane is configured via environment variables in the `.env` file:

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `DATABASE_URL` | PostgreSQL connection string for metadata | Yes | - |
| `SERVER_HOST` | Host to bind the API server | No | `0.0.0.0` |
| `SERVER_PORT` | Port for the API server | No | `3000` |
| `AKS_CLUSTER_NAME` | Name of your AKS cluster | Yes | - |
| `AKS_RESOURCE_GROUP` | Azure resource group containing AKS | Yes | - |
| `AKS_NAMESPACE` | Kubernetes namespace for deployments | No | `toygres` |

### Azure Authentication

The control plane uses Azure's `DefaultAzureCredential` which supports multiple authentication methods:

1. **Environment Variables**: Set `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`
2. **Managed Identity**: Automatically used when running in Azure
3. **Azure CLI**: Use `az login` for local development

## Next Steps

Now that the scaffolding is in place, you can begin implementing:

1. **Phase 2: Activities** - Implement the six core activities
2. **Phase 3: Orchestrations** - Implement the four orchestrations
3. **Phase 4: Control Plane Server** - Implement the API and worker
4. **Phase 5: Testing & Documentation** - Add comprehensive tests

See [plan.md](plan.md) for the detailed implementation roadmap.

## Troubleshooting

### Project doesn't compile

```bash
# Clean and rebuild
cargo clean
cargo build
```

### Can't connect to Azure

```bash
# Login via Azure CLI
az login

# Verify your subscription
az account show
```

### Can't connect to database

```bash
# Test database connection
psql "$DATABASE_URL" -c "SELECT 1"
```

### Kubernetes access issues

```bash
# Get AKS credentials
az aks get-credentials --resource-group <resource-group> --name <cluster-name>

# Verify kubectl access
kubectl cluster-info
```

## Resources

- [Duroxide Documentation](https://github.com/affandar/duroxide)
- [Duroxide-PG Documentation](https://github.com/affandar/duroxide-pg)
- [Azure SDK for Rust](https://github.com/Azure/azure-sdk-for-rust)
- [kube-rs Documentation](https://kube.rs/)

