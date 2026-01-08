---
name: aks-deployment
description: Deploying and debugging Toygres on AKS (Azure Kubernetes Service). Use when deploying, debugging pods, viewing logs, troubleshooting SSL, or managing Kubernetes resources.
---

# AKS Deployment & Debugging

## Deployment

```bash
# Full deploy with HTTPS
./deploy/deploy-to-aks.sh --https

# Just restart to pick up new images
kubectl rollout restart deployment/toygres-server -n toygres-system
kubectl rollout status deployment/toygres-server -n toygres-system
```

## Viewing Logs

```bash
# Server logs
kubectl logs -n toygres-system -l app.kubernetes.io/component=server -f

# UI logs
kubectl logs -n toygres-system -l app.kubernetes.io/component=ui -f

# Previous crashed pod
kubectl logs -n toygres-system <pod-name> --previous
```

## Pod Management

```bash
# List pods
kubectl get pods -n toygres-system

# Describe pod (see events, errors)
kubectl describe pod <pod-name> -n toygres-system

# Exec into pod
kubectl exec -it <pod-name> -n toygres-system -- /bin/sh

# Delete pod (will restart)
kubectl delete pod <pod-name> -n toygres-system
```

## Common Issues

### Pod CrashLoopBackOff
```bash
# Check logs for crash reason
kubectl logs <pod-name> -n toygres-system --previous

# Common causes:
# - DATABASE_URL not set or wrong
# - Missing secrets
# - Port already in use
```

### Image Not Updating
```bash
# Force pull latest image
kubectl rollout restart deployment/toygres-server -n toygres-system

# Or delete pod directly
kubectl delete pod -n toygres-system -l app.kubernetes.io/component=server
```

### SSL Certificate Issues
```bash
# Check cert-manager
kubectl get certificate -n toygres-system
kubectl describe certificate toygres-tls -n toygres-system

# Check ingress
kubectl get ingress -n toygres-system
kubectl describe ingress toygres-ingress -n toygres-system
```

## Local Testing Before Deploy

```bash
# Pause AKS server
kubectl scale deployment toygres-server -n toygres-system --replicas=0

# Run locally
./scripts/start-control-plane.sh

# Test at http://localhost:3000

# Resume AKS
kubectl scale deployment toygres-server -n toygres-system --replicas=1
```
