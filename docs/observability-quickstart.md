# Toygres Observability Quick Start

## 🎯 Goal

Get full observability (metrics + logs + dashboards) running locally in 5 minutes, with seamless deployment to AKS later.

## 📋 Prerequisites

- Docker and Docker Compose installed
- Toygres repository cloned
- PostgreSQL running (for toygres-server)

## 🚀 Step-by-Step Setup

### 1. Start the Observability Stack

```bash
cd /path/to/toygres

# Start Grafana, Prometheus, Loki, and OTLP Collector
./scripts/start-observability.sh
```

**What this does:**
- Starts 4 Docker containers (OTLP Collector, Prometheus, Loki, Grafana)
- Auto-provisions datasources in Grafana
- Loads the Toygres dashboard
- Exposes services on localhost

**Verify it's running:**
```bash
./scripts/observability-status.sh
```

### 2. Configure Toygres to Send Metrics

```bash
# Load observability environment variables
source observability/env.local.example

# Verify variables are set
env | grep DUROXIDE
```

You should see:
```
DUROXIDE_OBSERVABILITY_ENABLED=true
DUROXIDE_LOG_FORMAT=json
DUROXIDE_LOG_LEVEL=info
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
```

### 3. Start Toygres Server

```bash
# Make sure PostgreSQL is running first
# (see scripts/setup-db.sh)

# Run toygres server with observability
cargo run --bin toygres-server -- server
```

**Look for these log messages:**
```
INFO duroxide::runtime Duroxide observability enabled: metrics → http://localhost:4317, log_format = json
INFO toygres_server Starting server on 0.0.0.0:3000
```

### 4. Generate Some Activity

```bash
# Create a PostgreSQL instance
curl -X POST http://localhost:3000/api/instances \
  -H "Content-Type: application/json" \
  -d '{
    "user_name": "test",
    "name": "demo-pg",
    "password": "demo123",
    "dns_label": "demo"
  }'

# List instances
curl http://localhost:3000/api/instances

# Check orchestration status
curl http://localhost:3000/api/orchestrations/create-demo-pg
```

### 5. View Metrics in Grafana

1. Open Grafana: http://localhost:3001
2. Login: `admin` / `admin`
3. Navigate to: **Dashboards → Toygres → Toygres Overview**

**You'll see:**
- 📊 Orchestration start/completion rates
- ⏱️ Activity duration percentiles (DeployPostgres, WaitForReady, etc.)
- ✅ Success/failure rates
- 💾 Database operation latency
- 🚨 Infrastructure errors

### 6. Query Logs in Loki

1. In Grafana, click **Explore** (compass icon)
2. Select **Loki** datasource
3. Try these queries:

**All logs for an instance:**
```logql
{job="toygres"} | json | instance_id="create-demo-pg"
```

**Failed activities:**
```logql
{job="toygres"} | json | level="error"
```

**Activity execution timeline:**
```logql
{job="toygres"} | json | activity_name != ""
```

## 🔍 Understanding the Metrics

### Key Panels in the Dashboard

#### 1. Orchestrations Started
Shows how many `CreateInstance` and `DeleteInstance` orchestrations are starting per second.

#### 2. Orchestration Success Rate
Percentage of orchestrations completing successfully vs failing.

#### 3. Activity Duration Percentiles
- **p99**: 99th percentile (slowest 1%)
- **p95**: 95th percentile
- **p50**: Median

Watch for:
- `DeployPostgres` - Should be <5s
- `WaitForReady` - Varies based on pod startup (10-60s)
- `TestConnection` - Should be <1s

#### 4. Database Operation Latency
- **fetch** - How long to fetch orchestration work items
- **ack** - How long to acknowledge completion

Should be <100ms for healthy database.

#### 5. Infrastructure Errors
Spikes indicate:
- Database connectivity issues
- Transaction failures
- Provider errors

## 🛠️ Troubleshooting

### Metrics Not Showing Up

**Check 1: Is observability enabled?**
```bash
echo $DUROXIDE_OBSERVABILITY_ENABLED  # Should be "true"
```

**Check 2: Is OTLP Collector reachable?**
```bash
curl -v http://localhost:4317
# Should connect (even if it fails with empty response)
```

**Check 3: Are metrics being exported?**
```bash
# Check Prometheus scrape targets
open http://localhost:9090/targets
# Look for "otel-collector" - should be UP
```

**Check 4: Toygres logs**
```bash
cargo run --bin toygres-server -- server 2>&1 | grep observability
# Should see: "Duroxide observability enabled: ..."
```

### Dashboard is Empty

**Wait 15-30 seconds** after starting toygres-server for first metrics to appear.

If still empty:
1. Check Prometheus has data: http://localhost:9090/graph
2. Run query: `duroxide_client_orchestration_starts`
3. If empty, metrics aren't being exported (see above)

### Logs Not in Loki

**Issue**: JSON logs need to be written to `./logs` directory.

Current setup expects logs on stdout. To fix:
1. Redirect toygres output to logs directory:
   ```bash
   mkdir -p logs
   cargo run --bin toygres-server -- server > logs/toygres.log 2>&1 &
   ```

2. Logs are automatically sent via OTLP to OTEL Collector → Loki

## 📦 What Gets Deployed

```
Docker Containers:
├── toygres-otel-collector   (port 4317) - Receives OTLP metrics
├── toygres-prometheus        (port 9090) - Stores metrics
├── toygres-loki             (port 3100) - Stores logs
└── toygres-grafana          (port 3001) - Dashboards

Data Volumes:
├── prometheus-data          - Metrics retention
├── loki-data               - Log retention
└── grafana-data            - Dashboard state
```

## 🚀 Next Steps

### 1. Customize the Dashboard

Edit in Grafana UI and export:
```bash
# Export modified dashboard
curl -H "Authorization: Bearer <api-key>" \
  http://localhost:3001/api/dashboards/uid/toygres-overview \
  > observability/grafana/dashboards/toygres-overview.json
```

### 2. Add Alerting

In Grafana, create alerts for:
- High orchestration failure rate (>5%)
- Slow activities (>60s p99)
- Database latency spikes (>500ms p95)
- Infrastructure errors (>10/min)

### 3. Deploy to AKS

When ready for production:

```bash
# Option A: Use provided Kubernetes manifests
kubectl apply -f k8s/observability/

# Option B: Use Helm (recommended)
helm install observability-stack prometheus-community/kube-prometheus-stack \
  --namespace toygres
```

Update toygres deployment:
```bash
kubectl set env deployment/toygres-server \
  -n toygres \
  OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector.toygres.svc.cluster.local:4317 \
  DUROXIDE_OBSERVABILITY_ENABLED=true
```

**Same code, different endpoint!** 🎉

## 🧹 Cleanup

```bash
# Stop and remove all containers
./scripts/stop-observability.sh

# Stop and delete all data
./scripts/stop-observability.sh --clean
```

## 📚 Learn More

- [Full Observability README](../observability/README.md)
- [Duroxide Observability Guide](https://github.com/affandar/duroxide/blob/main/docs/observability-guide.md)
- [Example Queries](../observability/README.md#example-queries)

## 💡 Tips

1. **Use Compact format for development**:
   ```bash
   export DUROXIDE_LOG_FORMAT=compact
   ```
   More human-readable than JSON.

2. **Increase log level for debugging**:
   ```bash
   export DUROXIDE_LOG_LEVEL=debug
   export RUST_LOG=debug
   ```

3. **Monitor database performance**:
   The "Database Operation Latency" panel is your friend for debugging slow orchestrations.

4. **Trace specific instances**:
   Use the instance_id in Loki queries to see the full execution timeline of any orchestration.

## 🎊 Success!

You now have:
- ✅ Full OpenTelemetry metrics collection
- ✅ Structured JSON logs aggregated in Loki
- ✅ Pre-built Grafana dashboards
- ✅ Local development environment
- ✅ Production-ready configuration (for AKS)

Happy monitoring! 🚀📊


