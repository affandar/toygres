````skill
---
name: aks-deployment
description: Deploying and debugging Toygres on AKS (Azure Kubernetes Service). Use when deploying, debugging pods, viewing logs, troubleshooting SSL, or managing Kubernetes resources.
---

# AKS Deployment & Debugging

## Current Infrastructure

- **Ingress:** AKS Application Routing add-on (ingress class `webapprouting.kubernetes.azure.com`)
- **TLS:** cert-manager v1.19+ with Let's Encrypt ClusterIssuer (HTTP-01 solver)
- **Namespaces:** `toygres-system` (server, UI), `toygres` (PG instances), `app-routing-system` (ingress), `cert-manager`
- **Auth:** AAD-enabled cluster — requires `kubelogin convert-kubeconfig -l azurecli` after `az aks get-credentials`

## Build & Deploy

```bash
# Build server image
source .env
az acr login --name "$TOYGRES_ACR_NAME"
docker build --platform linux/amd64 -f deploy/Dockerfile.server -t "$TOYGRES_ACR_NAME.azurecr.io/toygres-server:latest" .
docker push "$TOYGRES_ACR_NAME.azurecr.io/toygres-server:latest"

# Rollout (server is in toygres-system, not toygres)
kubectl rollout restart deployment/toygres-server -n toygres-system
kubectl rollout status deployment/toygres-server -n toygres-system

# Verify new image
kubectl get pods -n toygres-system -o jsonpath='{.items[*].status.containerStatuses[*].imageID}'
```

## Viewing Logs

```bash
# Server logs (toygres-system namespace)
kubectl logs -n toygres-system -l app.kubernetes.io/component=server -f

# UI logs
kubectl logs -n toygres-system -l app.kubernetes.io/component=ui -f

# Instance pod logs (toygres namespace)
kubectl logs -n toygres <instance-pod-name>

# Previous crashed pod
kubectl logs -n toygres-system <pod-name> --previous
```

## Pod Management

```bash
# Control plane pods
kubectl get pods -n toygres-system

# Instance pods and services
kubectl get pods -n toygres
kubectl get svc -n toygres

# Describe pod (see events, errors)
kubectl describe pod <pod-name> -n toygres-system

# Exec into pod
kubectl exec -it <pod-name> -n toygres-system -- /bin/sh
```

## Ingress & TLS

```bash
# Check ingress (uses App Routing, not nginx)
kubectl get ingress -n toygres-system
kubectl describe ingress toygres-ingress -n toygres-system

# Check Let's Encrypt certificate
kubectl get certificate -n toygres-system
kubectl describe certificate -n toygres-system

# Check cert-manager ClusterIssuer
kubectl get clusterissuer letsencrypt-prod -o yaml

# App Routing controller pods
kubectl get pods -n app-routing-system
```

The App Routing add-on runs on the system node pool and manages its own nginx controller. HTTP-01 challenges require port 80 reachable from the internet on the App Routing LB IP.

## Common Issues

### Pod CrashLoopBackOff
```bash
kubectl logs <pod-name> -n toygres-system --previous
# Common causes: DATABASE_URL wrong, missing secrets, port conflict
```

### Image Not Updating
```bash
# Force pull latest
kubectl rollout restart deployment/toygres-server -n toygres-system
```

### Image Pull Errors (401 Unauthorized)
Check that the image references the correct ACR hostname. Hardcoded ACR names in code:
- `deploy_postgres.rs`, `deploy_postgres_v2.rs`, `deploy_postgres_from_pvc.rs` — `DEFAULT_PG_DURABLE_REGISTRY`
- `api.rs` — `TOYGRES_ACR_HOST` fallback

### Azure Workload Identity / azcopy 403 Errors

`azcopy login --identity` uses VM-based managed identity (IMDS), not AKS workload identity.

**Fix:** Use `--login-type=workload` explicitly:
```bash
azcopy login --login-type=workload
```

### LoadBalancer IP Timeout

LB external IP assignment can take 30-90+ seconds. Code that polls for LB IPs (`get_connection_strings.rs`) must wait at least 120 seconds. DNS propagation adds another 30-60 seconds on top.

```rust
// Sufficient iterations: 24 × 5s = 120s
for attempt in 1..=24 {
    // check for external IP
    tokio::time::sleep(Duration::from_secs(5)).await;
}
```

````
