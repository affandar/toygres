# Toygres Observability

Complete observability stack for Toygres using Duroxide's OpenTelemetry metrics and structured logging.

## Overview

This directory contains the full observability stack configuration for both **local development** (Docker Compose) and **production deployment** (AKS/Kubernetes).

**Stack Components:**
- **OpenTelemetry Collector** - Receives OTLP metrics/logs, exports to Prometheus/Loki
- **Prometheus** - Time-series metrics storage
- **Loki** - Log aggregation (receives logs via OTLP from OTEL Collector)
- **Grafana** - Dashboards and visualization

## Quick Start (Local Development)

### 1. Start the Observability Stack

```bash
# Start all services
./scripts/start-observability.sh

# Check status
./scripts/observability-status.sh
```

**Services will be available at:**
- Grafana: http://localhost:3001 (login: admin/admin)
- Prometheus: http://localhost:9090
- Loki: http://localhost:3100
- OTLP Collector: http://localhost:4317

### 2. Run Toygres with Observability Enabled

```bash
# Load observability environment variables
source observability/env.local.example

# Run toygres server
cargo run --bin toygres-server -- server
```

### 3. View Dashboards

Open Grafana at http://localhost:3001 and navigate to:
- **Dashboards → Toygres → Toygres Overview**

You'll see:
- Orchestration start/completion rates
- Activity duration percentiles
- Success/failure rates
- Database performance metrics
- Infrastructure error tracking

### 4. Stop the Stack

```bash
# Stop without removing data
./scripts/stop-observability.sh

# Stop and clean all volumes
./scripts/stop-observability.sh --clean
```

## Metrics Available

### Orchestration Metrics
- `duroxide_orchestration_completions` - Success/failure counts
- `duroxide_orchestration_failures` - Failures by error type
- `duroxide_orchestration_history_size_events` - History event count
- `duroxide_orchestration_turns` - Turns to completion

### Activity Metrics
- `duroxide_activity_executions` - Execution outcomes (success/app_error/system_error)
- `duroxide_activity_duration_ms` - Execution duration histogram
- `duroxide_activity_app_errors` - Application errors by activity

### Provider Metrics (Database Performance)
- `duroxide_provider_fetch_orchestration_item_duration_ms` - Fetch latency
- `duroxide_provider_ack_orchestration_item_duration_ms` - Ack latency
- `duroxide_provider_infrastructure_errors` - Database errors

### Client Metrics
- `duroxide_client_orchestration_starts` - Orchestrations started
- `duroxide_client_external_events_raised` - Events raised

## Example Queries

### Orchestration Success Rate
```promql
sum(rate(duroxide_orchestration_completions{status="success"}[5m])) 
/ 
sum(rate(duroxide_orchestration_completions[5m]))
```

### Activity Duration (p99)
```promql
histogram_quantile(0.99, 
  rate(duroxide_activity_duration_ms_bucket[5m])
) by (activity_name)
```

### Database Performance (p95 fetch latency)
```promql
histogram_quantile(0.95, 
  rate(duroxide_provider_fetch_orchestration_item_duration_ms_bucket[5m])
)
```

### Find Slow Instance Creations
```promql
histogram_quantile(0.99, 
  rate(duroxide_activity_duration_ms_bucket{
    activity_name=~"DeployPostgres|WaitForReady"
  }[5m])
)
```

## Log Queries (Loki)

Access Loki via Grafana's Explore tab, or query directly:

### All logs for an orchestration instance
```logql
{job="toygres"} | json | instance_id="order-123"
```

### Failed activities
```logql
{job="toygres"} | json | activity_name != "" | outcome="app_error"
```

### Error rate by orchestration
```logql
rate({job="toygres", level="error"}[5m]) by (orchestration_name)
```

## Configuration Files

```
observability/
├── otel-collector-config.yaml    # OTLP → Prometheus export
├── prometheus.yml                 # Prometheus scrape config
├── loki-config.yaml              # Loki storage and limits
├── grafana/
│   ├── provisioning/
│   │   ├── datasources/          # Auto-provision Prometheus & Loki
│   │   └── dashboards/           # Auto-load dashboards
│   └── dashboards/
│       └── toygres-overview.json # Main dashboard
└── env.local.example             # Local environment variables
```

## Deployment to AKS

### Option 1: Using Existing Kubernetes Manifests

The same configs can be deployed to AKS with minimal changes:

```bash
# Convert docker-compose to K8s manifests (if needed)
# Or use the provided k8s/ directory

kubectl apply -f k8s/observability/
```

### Option 2: Helm Chart (Recommended for Production)

```bash
# Use Grafana's official Helm charts
helm repo add grafana https://grafana.github.io/helm-charts
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts

# Install stack
helm install prometheus prometheus-community/kube-prometheus-stack \
  --namespace toygres --create-namespace

helm install loki grafana/loki-stack \
  --namespace toygres
```

### Environment Configuration for AKS

Apply the AKS environment variables as a ConfigMap:

```bash
kubectl create configmap toygres-observability \
  --from-env-file=observability/env.aks.example \
  -n toygres

# Update deployment to use ConfigMap
kubectl patch deployment toygres-server -n toygres \
  --patch '{"spec":{"template":{"spec":{"containers":[{"name":"toygres-server","envFrom":[{"configMapRef":{"name":"toygres-observability"}}]}]}}}}'
```

## Seamless Local ↔ AKS Transition

The configuration is designed to work in both environments:

**Local Development:**
- Docker Compose for easy setup
- Environment sourced from `env.local.example`
- Services on localhost

**AKS Production:**
- Kubernetes deployments
- Environment from ConfigMap/Secrets
- Services via cluster DNS (e.g., `otel-collector.toygres.svc.cluster.local`)

**Same Code, Different Endpoints:**
The toygres-server automatically picks up the environment:
- `OTEL_EXPORTER_OTLP_ENDPOINT` - Changes based on environment
- All other configs remain identical

## Troubleshooting

### Metrics Not Appearing

1. Check if observability is enabled:
   ```bash
   echo $DUROXIDE_OBSERVABILITY_ENABLED
   ```

2. Verify OTLP collector is reachable:
   ```bash
   curl -v http://localhost:4317
   ```

3. Check Prometheus targets:
   - Open http://localhost:9090/targets
   - Ensure `otel-collector` is UP

4. View toygres logs:
   ```bash
   # Should see: "Duroxide observability enabled: metrics → ..."
   cargo run --bin toygres-server -- server
   ```

### Grafana Dashboard Not Loading

1. Check if dashboards are provisioned:
   ```bash
   docker exec toygres-grafana ls /var/lib/grafana/dashboards
   ```

2. Verify datasources:
   - Open Grafana → Configuration → Data Sources
   - Both Prometheus and Loki should be present

### Logs Not in Loki

1. Ensure toygres is writing JSON logs:
   ```bash
   # Check DUROXIDE_LOG_FORMAT=json
   echo $DUROXIDE_LOG_FORMAT
   ```

2. Write logs to the correct directory:
   ```bash
   # Logs are exported via OTLP to OTEL Collector
   # Backup logs: ~/.toygres/server.log
   ```

## Advanced: Custom Dashboards

Create your own dashboards in Grafana and save them to `observability/grafana/dashboards/`:

```bash
# Export dashboard JSON from Grafana
curl -H "Authorization: Bearer <your-api-key>" \
  http://localhost:3001/api/dashboards/uid/my-dashboard > my-dashboard.json

# Save to dashboards directory
mv my-dashboard.json observability/grafana/dashboards/

# Restart Grafana to load
docker compose -f docker-compose.observability.yml restart grafana
```

## Performance Impact

- **Metrics Collection**: ~2-5% overhead
- **Log Shipping**: Minimal (async, batched)
- **Total**: <5% overhead in production

## References

- [Duroxide Observability Guide](https://github.com/affandar/duroxide/blob/main/docs/observability-guide.md)
- [OpenTelemetry Collector Docs](https://opentelemetry.io/docs/collector/)
- [Prometheus Query Docs](https://prometheus.io/docs/prometheus/latest/querying/basics/)
- [LogQL (Loki) Docs](https://grafana.com/docs/loki/latest/logql/)


