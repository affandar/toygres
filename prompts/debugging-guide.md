# Debugging Guide for Toygres

## Common Issues and Solutions

### Compilation Issues

#### Missing Dependencies
```bash
# Update dependencies
cargo update

# Clean build
cargo clean && cargo build
```

#### Duroxide Git Dependency Issues
```bash
# Force update git dependencies
cargo update -p duroxide
cargo update -p duroxide-pg
```

### Database Connection Issues

#### Can't Connect to Metadata Database
```bash
# Test connection
psql "$DATABASE_URL" -c "SELECT 1"

# Check if schema exists
psql "$DATABASE_URL" -c "\dt"
```

#### Schema Not Found
```bash
# Run the setup script
./scripts/setup-db.sh

# Or manually create schema
psql "$DATABASE_URL" -f scripts/setup-db.sh
```

### Kubernetes Issues

#### Can't Connect to AKS Cluster
```bash
# Get credentials
az aks get-credentials --resource-group <rg> --name <cluster> --overwrite-existing

# Verify connection
kubectl cluster-info

# Check namespace exists
kubectl get namespace toygres
```

#### Pods Not Starting
```bash
# Check pod status
kubectl get pods -n toygres

# Check pod logs
kubectl logs -n toygres <pod-name>

# Describe pod for events
kubectl describe pod -n toygres <pod-name>
```

### Duroxide Workflow Issues

#### Orchestration Not Starting
- Check that orchestrations are registered in the worker
- Verify the input serialization is correct
- Check worker logs for startup errors

#### Activity Failures
- Activities should be idempotent
- Check activity logs for specific errors
- Verify activity inputs are valid

#### Orchestration Stuck
- Check if waiting on a timer
- Verify activities are completing
- Look for deadlocks in activity logic

### Azure Authentication Issues

#### DefaultAzureCredential Fails
```bash
# Login via Azure CLI
az login

# Verify account
az account show

# Or use environment variables
export AZURE_CLIENT_ID="..."
export AZURE_CLIENT_SECRET="..."
export AZURE_TENANT_ID="..."
```

## Debugging Tools

### Logging

Enable detailed logging:
```bash
# Set log level via environment
export RUST_LOG=toygres_server=debug,toygres_activities=debug,duroxide=debug

# Or in .env
RUST_LOG=toygres_server=debug,toygres_activities=debug
```

### Database Queries

Check instance state:
```sql
-- List all instances
SELECT id, name, state, health_status, created_at FROM instances;

-- Check specific instance
SELECT * FROM instances WHERE name = 'my-instance';

-- Check Duroxide state (depends on duroxide-pg schema)
-- Check duroxide-pg docs for table names
```

### Kubernetes Debugging

```bash
# Get all resources in namespace
kubectl get all -n toygres

# Check events
kubectl get events -n toygres --sort-by='.lastTimestamp'

# Check persistent volumes
kubectl get pv,pvc -n toygres

# Execute into a pod
kubectl exec -it -n toygres <pod-name> -- psql -U postgres
```

## Testing Strategies

### Unit Testing Activities

Mock the Kubernetes client:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_deploy_activity() {
        // Mock K8s client
        // Test activity logic
    }
}
```

### Integration Testing Orchestrations

Use Duroxide in-memory provider:
```rust
#[tokio::test]
async fn test_create_instance_orchestration() {
    // Use in-memory Duroxide backend
    // Start orchestration
    // Verify output
}
```

### End-to-End Testing

```bash
# Use a local K8s cluster
kind create cluster

# Or minikube
minikube start

# Deploy and test
cargo run --bin toygres-server
curl -X POST http://localhost:3000/instances -d '{"name":"test",...}'
```

## Performance Issues

### Slow Orchestrations
- Check activity execution times
- Verify database query performance
- Look for unnecessary retries

### High Memory Usage
- Check for memory leaks in long-running orchestrations
- Verify resources are cleaned up after completion
- Monitor Duroxide workflow state size

### API Latency
- Add tracing to identify bottlenecks
- Check database connection pool settings
- Verify Kubernetes API calls are efficient

## Useful Commands

```bash
# Check project builds
cargo check --workspace

# Run linter
cargo clippy --workspace

# Format code
cargo fmt --workspace

# Run all tests
cargo test --workspace

# Build release version
cargo build --release

# Check for outdated dependencies
cargo outdated
```

## Getting Help

1. Check Duroxide documentation: https://github.com/affandar/duroxide
2. Check Duroxide-PG documentation: https://github.com/affandar/duroxide-pg
3. Review the implementation plan: `docs/plan.md`
4. Search existing issues or error messages

