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

## Smoke Test

After a deploy, run this end-to-end smoke test to verify the control plane works:

```bash
BASE="https://toygres-aks-ui.westus3.cloudapp.azure.com"

# 1. Login (POST with form data — NOT a GET query param)
COOKIE=$(curl -sk -D- -o/dev/null -X POST "$BASE/login" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "username=admin&password=toygres2025" 2>&1 | grep -i set-cookie | head -1)
SESSION="toygres_session=authenticated_toygres_admin_session"

# 2. Health check
curl -sk -b "$SESSION" "$BASE/api/health"

# 3. Create test instance (password field is required)
SMOKE="smoke$(date +%m%d%H%M%S)"
curl -sk -b "$SESSION" -X POST "$BASE/api/instances" \
  -H "Content-Type: application/json" \
  -d "{\"name\": \"$SMOKE\", \"password\": \"smoketest123\"}"

# 4. Poll for running + healthy (takes 1-3 minutes)
for i in $(seq 1 36); do
  sleep 10
  STATE=$(curl -sk -b "$SESSION" "$BASE/api/instances" 2>/dev/null | \
    python3 -c "import json,sys; d=json.load(sys.stdin); m=[i for i in d if i.get('user_name')=='$SMOKE']; print(json.dumps({'state':m[0].get('state','?'),'healthy':m[0].get('health_status')}) if m else 'not_found')" 2>/dev/null)
  echo "attempt $i: $STATE"
  echo "$STATE" | grep -q '"healthy": "healthy"' && echo "SUCCESS!" && break
done

# 5. Delete test instance (use user_name, not k8s_name)
curl -sk -b "$SESSION" -X DELETE "$BASE/api/instances/$SMOKE"

# 6. Wait for deletion
for i in 1 2 3 4 5 6; do
  sleep 10
  GONE=$(curl -sk -b "$SESSION" "$BASE/api/instances" 2>/dev/null | \
    python3 -c "import json,sys; d=json.load(sys.stdin); print('gone' if not [i for i in d if i.get('user_name')=='$SMOKE'] else 'still_there')" 2>/dev/null)
  echo "$GONE"
  [ "$GONE" = "gone" ] && echo "Cleanup complete!" && break
done
```

**Common issues:**
- Login is `POST /login` with `Content-Type: application/x-www-form-urlencoded` and fields `username` + `password`. A GET to `/login` returns the HTML form page.
- Create instance requires a `password` field in the JSON body (not just `name`).
- Delete uses the `user_name` (e.g., `smoke0314044056`), not the `k8s_name` (e.g., `smoke0314044056-1854238c`).
- Health status may stay `null` for several minutes while the instance actor starts its first health check cycle. With 291+ existing actors, worker capacity is heavily loaded.
- Credentials: `TOYGRES_ADMIN_USERNAME=admin`, `TOYGRES_ADMIN_PASSWORD=toygres2025`

