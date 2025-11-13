# Toygres Examples

This directory contains example programs for Toygres development.

## manual_deploy.rs

**Phase 0 Proof of Concept** - Deploys PostgreSQL to AKS with a public IP and validates the deployment.

### What It Does

This example deploys a complete PostgreSQL instance to your AKS cluster:

1. Connects to your AKS cluster using kube-rs
2. Queries cluster-level resources:
   - Nodes (if you have permissions)
   - Namespaces
   - Storage classes
3. Queries namespace-scoped resources:
   - Pods
   - Services
   - StatefulSets
   - PersistentVolumeClaims
4. Reports what we can and cannot access

### Why This First?

Before building deployment logic, we need to understand:
- ✅ Can we connect to the cluster?
- ✅ What permissions do we have?
- ✅ What storage classes are available?
- ✅ Does our target namespace exist?
- ✅ Can we query the resources we'll need to manage?

### Prerequisites

- Azure CLI installed and logged in (`az login`)
- kubectl configured with your AKS cluster credentials
- (Optional) An AKS cluster with a `toygres` namespace

### Setup AKS Access

If you haven't already set up your AKS cluster:

```bash
# Run the infrastructure setup script
./scripts/setup-infra.sh

# Or manually get credentials
az aks get-credentials --resource-group <your-rg> --name <your-cluster>
```

Verify kubectl access:

```bash
kubectl cluster-info
kubectl get nodes
```

### Running the Example

**Basic usage** (deploys with public IP):

```bash
cargo run --example manual_deploy
```

**Use cluster-internal IP only** (ClusterIP service):

```bash
USE_LOAD_BALANCER=false cargo run --example manual_deploy
```

**Custom configuration**:

```bash
INSTANCE_NAME=my-postgres POSTGRES_PASSWORD=mypass123 cargo run --example manual_deploy
```

**With debug logging**:

```bash
RUST_LOG=debug cargo run --example manual_deploy
```

### Expected Output

```
=== AKS Connectivity and Permissions Test ===

Connecting to Kubernetes cluster...
✓ Connected to Kubernetes cluster

--- Cluster Information ---
Querying cluster nodes...
  ✓ Found 3 nodes
    - aks-nodepool1-12345678-vmss000000: v1.28.3 (Ubuntu 22.04.3 LTS)
    - aks-nodepool1-12345678-vmss000001: v1.28.3 (Ubuntu 22.04.3 LTS)
    - aks-nodepool1-12345678-vmss000002: v1.28.3 (Ubuntu 22.04.3 LTS)

--- Namespaces ---
Listing namespaces...
  ✓ Found 8 namespaces
    - default
    - kube-node-lease
    - kube-public
    - kube-system
    - toygres
  ✓ 'toygres' namespace exists

--- Storage Classes ---
Querying available storage classes...
  ✓ Found 4 storage classes
    - azurefile: file.csi.azure.com
    - azurefile-csi: file.csi.azure.com
    - azurefile-csi-premium: file.csi.azure.com
    - default: disk.csi.azure.com (default)
    - managed-csi: disk.csi.azure.com
    - managed-csi-premium: disk.csi.azure.com

--- Resources in namespace 'toygres' ---
  ✓ Namespace 'toygres' exists and is accessible
Querying pods...
  ✓ Found 0 pods
Querying services...
  ✓ Found 0 services
Querying statefulsets...
  ✓ Found 0 statefulsets
Querying persistent volume claims...
  ✓ Found 0 PVCs

=== Test Complete ===
Summary:
  ✓ Can connect to AKS cluster
  ✓ Can query cluster-level resources
  ✓ Can query namespace-scoped resources
  ✓ Have sufficient permissions for Toygres operations
```

### What This Tells Us

#### ✅ Success Indicators

- **Connected**: We can authenticate to the cluster
- **List nodes**: We have cluster-level read access (or graceful degradation if not)
- **List namespaces**: We can see available namespaces
- **List storage classes**: We can see what storage options are available
- **Query namespace resources**: We can list pods, services, StatefulSets, PVCs

#### ⚠️ Potential Issues

If you see errors:

**"Cannot list nodes"**
- OK if you're not cluster-admin
- Not required for Toygres operations

**"Cannot list namespaces"**
- May need broader RBAC permissions
- Can work around by hardcoding namespace

**"Cannot access namespace 'toygres'"**
- Namespace doesn't exist: `kubectl create namespace toygres`
- Insufficient permissions: Check RBAC roles

**"Cannot list pods/services/statefulsets/PVCs"**
- Need read/write permissions in the namespace
- Check your service account's RBAC bindings

### Troubleshooting

#### "Failed to create Kubernetes client"
```bash
# Check kubectl configuration
kubectl cluster-info

# Get AKS credentials
az aks get-credentials --resource-group <rg> --name <cluster> --overwrite-existing

# Verify you can access the cluster
kubectl get nodes
```

#### Insufficient Permissions

If you see permission errors, you may need to update your RBAC roles:

```bash
# Check your current permissions
kubectl auth can-i --list --namespace=toygres

# Example: Grant admin access to toygres namespace (for testing)
kubectl create rolebinding toygres-admin \
  --clusterrole=admin \
  --user=<your-email> \
  --namespace=toygres
```

### Next Steps

Once this test passes:

1. **Validate the output**: Do we have all necessary permissions?
2. **Check storage classes**: Is there a suitable default class?
3. **Uncomment deployment code**: Restore the full PostgreSQL deployment logic
4. **Test actual deployment**: Deploy a PostgreSQL instance
5. **Move to Phase 1**: Extract into reusable modules

### Deployment Code

The actual deployment code (creating StatefulSets, PVCs, Services) is currently commented out in the file. Once we've validated our permissions, we can uncomment it and test the full deployment flow.

### What We're Testing

```rust
// Cluster-level queries (via Api::all)
- Nodes
- Namespaces  
- StorageClasses

// Namespace-scoped queries (via Api::namespaced)
- Pods
- Services
- StatefulSets
- PersistentVolumeClaims
```

All of these will be needed for Toygres operations, so it's important to verify we can query them before attempting to create them.
