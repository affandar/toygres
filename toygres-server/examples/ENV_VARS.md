# Environment Variables for manual_deploy.rs

| Variable | Default | Description |
|----------|---------|-------------|
| `AKS_NAMESPACE` | `toygres` | Kubernetes namespace for deployment |
| `INSTANCE_NAME` | `test-pg-1` | Name for the PostgreSQL instance |
| `POSTGRES_PASSWORD` | `testpass123` | Password for PostgreSQL |
| `USE_LOAD_BALANCER` | `true` | Use LoadBalancer service (public IP). Set to `false` for ClusterIP (internal only) |
| `RUST_LOG` | `manual_deploy=info` | Logging level (`debug`, `info`, `warn`, `error`) |

## Examples

**Deploy with public IP (default):**
```bash
cargo run --example manual_deploy
```

**Deploy with cluster-internal IP only:**
```bash
USE_LOAD_BALANCER=false cargo run --example manual_deploy
```

**Custom instance name and password:**
```bash
INSTANCE_NAME=my-db POSTGRES_PASSWORD=securepass456 cargo run --example manual_deploy
```

**Deploy to different namespace:**
```bash
AKS_NAMESPACE=production cargo run --example manual_deploy
```

**All options combined:**
```bash
AKS_NAMESPACE=production \
INSTANCE_NAME=prod-db-1 \
POSTGRES_PASSWORD=super-secure-password \
USE_LOAD_BALANCER=true \
RUST_LOG=debug \
cargo run --example manual_deploy
```

