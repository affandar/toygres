# Toygres Logging Setup

## Overview

Toygres uses a **dual logging approach**:

1. **OTLP Export** - Native OpenTelemetry logs to OTEL Collector → Loki (primary)
2. **File Output** - JSON-structured logs to `~/.toygres/server.log` (backup/debugging)

## Architecture

```
toygres-server
    ├─> OTLP gRPC → OTEL Collector → Loki (primary)
    └─> ~/.toygres/server.log (JSON, backup for debugging)
```

## Why OTLP Direct Export?

We use OTLP for both **metrics** and **logs**:

- ✅ **Native protocol** - OpenTelemetry standard
- ✅ **Correlation** - Logs, traces, and metrics use same infrastructure
- ✅ **Efficient** - Binary protocol with batching
- ✅ **Unified pipeline** - Single OTEL Collector handles everything
- ✅ **No Promtail needed** - Direct OTLP export replaces file scraping
- ✅ **File backup** - Still have `server.log` for debugging

## Log Locations

- **Development**: `~/.toygres/server.log`
- **Production/AKS**: `/var/log/toygres/server.log` (or container stdout)

## Log Format

### File Output (JSON)
```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "level": "INFO",
  "target": "toygres_server",
  "message": "Starting Toygres server",
  "fields": {
    "instance_id": "abc-123",
    "orchestration_name": "create-instance"
  }
}
```

## Configuration

### Environment Variables

```bash
# Basic logging (via RUST_LOG)
export RUST_LOG=info,toygres_server=debug

# Duroxide internal logging configuration
export DUROXIDE_LOG_FORMAT=json  # json, compact, or pretty
export DUROXIDE_LOG_LEVEL=info
```

### OTEL Collector Configuration

OTEL Collector is configured to receive logs via OTLP and forward to Loki:

```yaml
# observability/otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317

exporters:
  otlphttp/loki:
    endpoint: "http://loki:3100/otlp"

service:
  pipelines:
    logs:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlphttp/loki]
```

## Testing the Setup

### 1. Start the Observability Stack

```bash
docker-compose -f docker-compose.observability.yml up -d
```

### 2. Start Toygres Server

```bash
cargo run --bin toygres-server -- standalone --port 3000
```

### 3. Verify Logs are Being Written

```bash
# Check console output (you should see logs in terminal)

# Check file output
tail -f ~/.toygres/server.log

# Should see JSON-formatted logs like:
# {"timestamp":"2024-...","level":"INFO","message":"..."}
```

### 4. Verify OTEL Collector is Running

Check OTEL Collector health:
```bash
curl http://localhost:13133
```

### 5. Query Logs in Loki

```bash
# Via API
curl -G http://localhost:3100/loki/api/v1/query_range \
  --data-urlencode 'query={job="toygres"}' \
  --data-urlencode 'limit=10'

# Via Grafana
# Go to http://localhost:3001
# Explore → Loki → {job="toygres"}
```

## Querying Logs in Grafana

### Common LogQL Queries

```logql
# All toygres logs
{job="toygres"}

# Filter by level
{job="toygres"} |= "ERROR"

# Filter by orchestration
{job="toygres"} | json | orchestration_name="create-instance"

# Count errors per minute
count_over_time({job="toygres"} |= "ERROR" [1m])
```

## Troubleshooting

### No logs in Loki?

1. **Check file exists**:
   ```bash
   ls -la ~/.toygres/server.log
   ```

2. **Check OTEL Collector logs**:
   ```bash
   # Check if OTEL Collector is receiving logs
   docker logs toygres-otel-collector
   ```

3. **Check OTEL Collector → Loki connection**:
   ```bash
   curl http://localhost:3100/ready
   ```

### Logs not formatted as JSON?

Check that `DUROXIDE_LOG_FORMAT=json` is set in your environment.

### Want to change log verbosity?

```bash
# More verbose
export RUST_LOG=trace,toygres_server=trace

# Less verbose
export RUST_LOG=warn,toygres_server=info
```

## Production Considerations

### AKS/Kubernetes

For production deployments:

1. **Deploy OTEL Collector** - As a DaemonSet or Deployment
2. **Configure endpoint** - Point to OTEL Collector service
3. **Persistent volume** - Optional for file backup
4. **Use Loki in cluster** - Deploy Loki for log storage

### Log Retention

Configure Loki retention in `observability/loki-config.yaml`:

```yaml
limits_config:
  retention_period: 744h  # 31 days
```

## OTLP Log Export Details

The current implementation uses:

- **opentelemetry-otlp 0.27** - OTLP exporter
- **opentelemetry-appender-tracing 0.27** - Bridge between `tracing` and OpenTelemetry
- **opentelemetry_sdk 0.27** - SDK with batching support

Logs flow through:
1. Application logs via `tracing` macros
2. `OpenTelemetryTracingBridge` captures logs
3. Batched and sent via gRPC to OTEL Collector
4. OTEL Collector forwards to Loki via OTLP HTTP

This provides the same correlation and observability as traces and metrics!

