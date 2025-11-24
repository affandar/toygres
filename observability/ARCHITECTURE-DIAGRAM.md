# Toygres Observability Architecture

## Complete Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TOYGRES SERVER                                     │
│                         (Rust Application)                                   │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │                      Application Code                               │    │
│  │                                                                      │    │
│  │  • toygres-server (API, orchestration runner)                      │    │
│  │  • toygres-orchestrations (workflows)                              │    │
│  │  • toygres-activities (K8s operations)                             │    │
│  │                                                                      │    │
│  │  Uses: tracing::info!(), tracing::debug!(), etc.                   │    │
│  └───────────────┬──────────────────────────────────────────────────────┘  │
│                  │                                                           │
│                  ▼                                                           │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │              TRACING SUBSCRIBER (main.rs)                           │    │
│  │                                                                      │    │
│  │  ┌──────────────────────┐  ┌──────────────────────┐                │    │
│  │  │ OpenTelemetry Bridge │  │   File Layer         │                │    │
│  │  │   (OTLP Export)      │  │ (~/.toygres/         │                │    │
│  │  │                      │  │   server.log)        │                │    │
│  │  └──────────┬───────────┘  └──────────────────────┘                │    │
│  └─────────────┼──────────────────────────────────────────────────────┘    │
│                │                                                             │
│                │ LOGS via OTLP/gRPC                                          │
└────────────────┼─────────────────────────────────────────────────────────────┘
                 │
                 │
┌────────────────┼─────────────────────────────────────────────────────────────┐
│                │                                                              │
│  ┌─────────────▼──────────────────────────────────────────────────────┐    │
│  │                      DUROXIDE RUNTIME                               │    │
│  │                  (Workflow Engine)                                  │    │
│  │                                                                      │    │
│  │  ObservabilityConfig:                                               │    │
│  │  • metrics_enabled: true                                            │    │
│  │  • metrics_export_endpoint: http://localhost:4317                  │    │
│  │  • metrics_export_interval_ms: 10000                               │    │
│  │                                                                      │    │
│  │  Emits:                                                              │    │
│  │  • duroxide_orchestration_*                                         │    │
│  │  • duroxide_activity_*                                              │    │
│  │  • duroxide_worker_*                                                │    │
│  │  • duroxide_storage_*                                               │    │
│  └──────────────┬───────────────────────────────────────────────────────┘  │
│                 │                                                            │
│                 │ METRICS via OTLP/gRPC                                      │
└─────────────────┼────────────────────────────────────────────────────────────┘
                  │
                  │
        ┌─────────┴─────────┐
        │                   │
        │  Port 4317        │  Port 4318
        │  (gRPC)           │  (HTTP)
        ▼                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    OPENTELEMETRY COLLECTOR                                   │
│                    (otel/opentelemetry-collector)                            │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │                         RECEIVERS                                   │    │
│  │                                                                      │    │
│  │  • otlp/grpc  (0.0.0.0:4317)  ← Metrics + Logs                     │    │
│  │  • otlp/http  (0.0.0.0:4318)  ← Backup HTTP endpoint               │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                   │                                          │
│                                   ▼                                          │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │                        PROCESSORS                                   │    │
│  │                                                                      │    │
│  │  • batch:                                                           │    │
│  │    - timeout: 10s                                                   │    │
│  │    - send_batch_size: 1024                                          │    │
│  │                                                                      │    │
│  │  (Batches data for efficient export)                               │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                   │                                          │
│                    ┌──────────────┴──────────────┐                          │
│                    │                              │                          │
│                    ▼                              ▼                          │
│  ┌─────────────────────────────┐   ┌─────────────────────────────┐         │
│  │   METRICS PIPELINE          │   │    LOGS PIPELINE             │         │
│  │                             │   │                              │         │
│  │   Exporters:                │   │   Exporters:                 │         │
│  │   • prometheus              │   │   • otlphttp/loki            │         │
│  │     (port 8889)             │   │     (http://loki:3100/otlp) │         │
│  │   • debug (sampling)        │   │   • debug (sampling)         │         │
│  └──────────────┬──────────────┘   └──────────────┬───────────────┘        │
│                 │                                  │                         │
└─────────────────┼──────────────────────────────────┼─────────────────────────┘
                  │                                  │
                  │ Prometheus scrape format         │ OTLP/HTTP
                  │ (port 8889)                      │ (port 3100)
                  │                                  │
                  ▼                                  ▼
┌─────────────────────────────┐    ┌─────────────────────────────┐
│        PROMETHEUS            │    │          LOKI                │
│    (Time-Series DB)          │    │     (Log Aggregation)        │
│                              │    │                              │
│  Scrapes:                    │    │  Receives:                   │
│  • OTEL Collector :8889      │    │  • OTLP logs via HTTP        │
│    every 15s                 │    │                              │
│                              │    │  Stores:                     │
│  Stores:                     │    │  • Structured logs           │
│  • duroxide_orchestration_*  │    │  • With labels:              │
│  • duroxide_activity_*       │    │    - service.name            │
│  • duroxide_worker_*         │    │    - level                   │
│  • duroxide_storage_*        │    │    - instance_id             │
│                              │    │    - orchestration_name      │
│  Query with: PromQL          │    │                              │
│  Port: 9090                  │    │  Query with: LogQL           │
│                              │    │  Port: 3100                  │
└──────────────┬───────────────┘    └──────────────┬───────────────┘
               │                                   │
               │                                   │
               └──────────────┬────────────────────┘
                              │
                              │ Both datasources
                              ▼
                  ┌─────────────────────────────┐
                  │         GRAFANA              │
                  │   (Visualization Layer)      │
                  │                              │
                  │  Datasources:                │
                  │  • Prometheus (metrics)      │
                  │  • Loki (logs)               │
                  │                              │
                  │  Dashboards:                 │
                  │  • Toygres Overview          │
                  │  • Orchestration Metrics     │
                  │  • Activity Performance      │
                  │  • Worker Health             │
                  │  • Storage Operations        │
                  │                              │
                  │  Explore:                    │
                  │  • Query metrics (PromQL)    │
                  │  • Query logs (LogQL)        │
                  │  • Correlate logs + metrics  │
                  │                              │
                  │  Port: 3001                  │
                  │  URL: http://localhost:3001  │
                  └──────────────────────────────┘
```

## Data Flow Summary

### Metrics Path
```
Duroxide Runtime
  → OTLP gRPC (port 4317)
    → OTEL Collector (batch processor)
      → Prometheus Exporter (port 8889)
        → Prometheus (scrape every 15s)
          → Grafana (PromQL queries)
```

### Logs Path
```
Application (tracing macros)
  → OpenTelemetry Bridge
    → OTLP gRPC (port 4317)
      → OTEL Collector (batch processor)
        → OTLP HTTP Exporter
          → Loki (port 3100)
            → Grafana (LogQL queries)
```

### Backup Logs Path
```
Application (tracing macros)
  → File Layer
    → ~/.toygres/server.log (JSON)
      → (Can be scraped by Promtail if needed)
```

## Port Reference

| Service          | Port | Protocol | Purpose                    |
|------------------|------|----------|----------------------------|
| Toygres Server   | 3000 | HTTP     | REST API                   |
| OTEL Collector   | 4317 | gRPC     | OTLP receiver (primary)    |
| OTEL Collector   | 4318 | HTTP     | OTLP receiver (backup)     |
| OTEL Collector   | 8889 | HTTP     | Prometheus metrics export  |
| Prometheus       | 9090 | HTTP     | Metrics storage/query      |
| Loki             | 3100 | HTTP     | Log storage/query          |
| Grafana          | 3001 | HTTP     | Visualization dashboard    |

## Key Features

### Unified Pipeline
- **Single collector** handles both metrics and logs
- **Same protocol** (OTLP) for both data types
- **Consistent configuration** across observability signals

### Correlation
- Logs and metrics share common labels:
  - `service.name` = "toygres"
  - `service.version` = version from Cargo.toml
  - `instance_id`, `orchestration_name`, `activity_name` (in Duroxide logs)

### Efficiency
- **Batching** reduces network overhead (10s batches, 1024 items)
- **Binary protocol** (gRPC) for efficient transport
- **Local aggregation** in OTEL Collector before export

### Reliability
- **Multiple outputs**: OTLP + console + file
- **Graceful degradation**: If OTLP fails, file logs remain
- **Debug output**: OTEL Collector includes debug exporter for troubleshooting

## Configuration Files

- **OTEL Collector**: `observability/otel-collector-config.yaml`
- **Prometheus**: `observability/prometheus.yml`
- **Loki**: `observability/loki-config.yaml`
- **Grafana**: `observability/grafana/dashboards/`
- **Docker Compose**: `docker-compose.observability.yml`

## Environment Variables

```bash
# Toygres Server
export DUROXIDE_OBSERVABILITY_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export DUROXIDE_LOG_FORMAT=json
export DUROXIDE_LOG_LEVEL=info
export RUST_LOG=info,toygres_server=debug

# Service metadata
export OTEL_SERVICE_NAME=toygres
export OTEL_SERVICE_VERSION=0.1.0
```

## Query Examples

### Metrics (PromQL in Grafana)

```promql
# Orchestration execution rate
rate(duroxide_orchestration_executions_total[5m])

# Activity retry rate
rate(duroxide_activity_retries_total[5m])

# P95 activity duration
histogram_quantile(0.95, duroxide_activity_duration_seconds_bucket)

# Failed orchestrations
sum(rate(duroxide_orchestration_failures_total[5m])) by (orchestration_name)
```

### Logs (LogQL in Grafana)

```logql
# All toygres logs
{service_name="toygres"}

# Error logs only
{service_name="toygres"} | json | level="ERROR"

# Logs for specific orchestration
{service_name="toygres"} | json | orchestration_name="create-instance"

# Count errors per minute
count_over_time({service_name="toygres"} | json | level="ERROR" [1m])
```

## Next Steps: Traces

The infrastructure is ready for **distributed tracing**:

```
Application (tracing spans)
  → OTLP gRPC (port 4317)
    → OTEL Collector
      → Jaeger/Tempo
        → Grafana (trace visualization)
```

This would enable:
- End-to-end request tracing
- Span timing and dependencies
- Correlation with logs and metrics
- Full observability stack!

