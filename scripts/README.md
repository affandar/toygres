# Toygres Scripts

This directory contains helper scripts for managing Toygres infrastructure and deployments.

## Infrastructure Scripts

### setup-infra.sh

Creates the foundational Azure infrastructure for Toygres.

**What it does:**
- Creates Azure Resource Group
- Creates AKS cluster
- Configures kubectl
- Creates toygres namespace
- Sets up storage class

**Usage:**
```bash
./scripts/setup-infra.sh
```

The script will prompt you for configuration values.

### db-init.sh

Applies the initial migration for the CMS schema (version `0001`) and prepares
the Duroxide schema.

**What it does:**
- Ensures the `toygres_cms` schema exists
- Applies `migrations/cms/0001_initial_schema.sql` exactly once
- Creates the migration tracking table (`toygres_cms._toygres_migrations`)
- Cascades into `db-migrate.sh` so any newer migrations run automatically

**Usage:**
```bash
./scripts/db-init.sh
```

Requires `DATABASE_URL` in `.env`.  
`scripts/setup-db.sh` remains as a backward-compatible wrapper.

### db-migrate.sh

Applies any migrations numbered `0002` and above (none yet, but ready).

```bash
./scripts/db-migrate.sh
```

Migrations are stored in `migrations/cms/` and tracked in
`toygres_cms._toygres_migrations`.

### drop-duroxide-schema.sh

Drops the `toygres_duroxide` schema (all orchestration state). Prompts for confirmation.

```bash
./scripts/drop-duroxide-schema.sh
```

### drop-cms-schema.sh

Drops the `toygres_cms` schema (all Toygres metadata). Prompts for confirmation.

```bash
./scripts/drop-cms-schema.sh
```

### drop-all-schemas.sh

Drops both schemas in one shot. **Destructive** – wipes orchestrations and CMS data.

```bash
./scripts/drop-all-schemas.sh
```

## Deployment Management Scripts

### list-deployments.sh

Lists all PostgreSQL deployments managed by Toygres.

**Usage:**
```bash
./scripts/list-deployments.sh
```

**Output:**
- StatefulSets (PostgreSQL instances)
- Services (network endpoints)
- PersistentVolumeClaims (storage)
- Pods (running containers)

### cleanup-deployments.sh

Removes ALL PostgreSQL deployments in the toygres namespace.

**Usage:**
```bash
./scripts/cleanup-deployments.sh
```

**What it does:**
- Finds all StatefulSets with `app=postgres` label
- Shows list and asks for confirmation
- Deletes StatefulSet, Service, and PVC for each instance

**⚠️ Warning:** This will delete all data! Make sure you have backups if needed.

### cleanup-single.sh

Removes a single PostgreSQL deployment by name.

**Usage:**
```bash
./scripts/cleanup-single.sh <instance-name>
```

**Example:**
```bash
./scripts/cleanup-single.sh test-pg-1
```

**What it does:**
- Verifies the instance exists
- Asks for confirmation
- Deletes StatefulSet, Service, and PVC

## Environment Variables

All scripts respect environment variables from `.env`:

- `AKS_NAMESPACE` - Kubernetes namespace (default: `toygres`)
- `AKS_CLUSTER_NAME` - AKS cluster name
- `AKS_RESOURCE_GROUP` - Azure resource group
- `DATABASE_URL` - Metadata database connection string

## Quick Reference

```bash
# List what's deployed
./scripts/list-deployments.sh

# Clean up everything
./scripts/cleanup-deployments.sh

# Clean up one instance
./scripts/cleanup-single.sh test-pg-1

# Set up infrastructure
./scripts/setup-infra.sh

# Set up metadata database
./scripts/db-init.sh     # (setup-db.sh still works as a wrapper)
./scripts/db-migrate.sh  # Apply future migrations

# Drop schemas (danger!)
./scripts/drop-cms-schema.sh
./scripts/drop-duroxide-schema.sh
./scripts/drop-all-schemas.sh
```

## Examples

### Check current deployments before cleanup

```bash
# See what's running
./scripts/list-deployments.sh

# Output:
# StatefulSets:
#   NAME        READY   AGE
#   test-pg-1   1/1     5m
#   test-pg-2   1/1     2m
```

### Clean up a specific instance

```bash
./scripts/cleanup-single.sh test-pg-1

# Prompts for confirmation:
# Delete this deployment? (y/n): y
# 
# Deleting resources...
# ✓ StatefulSet deleted
# ✓ Service deleted
# ✓ PVC deleted
```

### Clean up everything

```bash
./scripts/cleanup-deployments.sh

# Shows all instances and prompts:
# Found the following deployments:
#   - test-pg-1
#   - test-pg-2
# 
# Delete all these deployments? (y/n): y
```

## Troubleshooting

### "Namespace 'toygres' does not exist"

Create the namespace:
```bash
kubectl create namespace toygres
```

### PVC won't delete (stuck in Terminating)

Pods might still be using it. Force delete:
```bash
kubectl delete pvc -n toygres <pvc-name> --grace-period=0 --force
```

### Can't connect to cluster

Get credentials:
```bash
az aks get-credentials --resource-group <rg> --name <cluster>
```

## Safety Features

All cleanup scripts:
- ✅ Ask for confirmation before deleting
- ✅ Show what will be deleted
- ✅ Only delete resources with `app=postgres` label
- ✅ Provide clear output about what was deleted

## Notes

- **Data Loss**: Deleting PVCs will permanently delete PostgreSQL data
- **LoadBalancers**: Services with LoadBalancer type may take a minute to fully delete
- **Pods**: StatefulSet deletion automatically deletes pods
- **Cleanup Order**: Scripts delete in the correct order (StatefulSet → Service → PVC)

