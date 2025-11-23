# Toygres Observability Architecture Diagram

## High-Level Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         TOYGRES APPLICATION                              │
│                                                                           │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    Toygres Server Process                         │  │
│  │                                                                    │  │
│  │  ┌────────────────────────────────────────────────────────────┐ │  │
│  │  │              Duroxide Runtime                               │ │  │
│  │  │                                                              │ │  │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │ │  │
│  │  │  │Orchestration │  │Orchestration │  │Orchestration │    │ │  │
│  │  │  │   Worker 0   │  │   Worker 1   │  │   Worker 2   │    │ │  │
│  │  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘    │ │  │
│  │  │         │                  │                  │             │ │  │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │ │  │
│  │  │  │  Activity    │  │  Activity    │  │  Activity    │    │ │  │
│  │  │  │  Worker 0    │  │  Worker 1    │  │  Worker 2    │    │ │  │
│  │  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘    │ │  │
│  │  │         │                  │                  │             │ │  │
│  │  │         └──────────────────┴──────────────────┘             │ │  │
│  │  │                            │                                 │ │  │
│  │  │                  ┌─────────▼──────────┐                    │ │  │
│  │  │                  │ Metrics Collector  │                    │ │  │
│  │  │                  │                    │                    │ │  │
│  │  │                  │ OpenTelemetry SDK  │                    │ │  │
│  │  │                  │ - Counters         │                    │ │  │
│  │  │                  │ - Histograms       │                    │ │  │
│  │  │                  │ - In-memory batch  │                    │ │  │
│  │  │                  └─────────┬──────────┘                    │ │  │
│  │  └──────────────────────────────┼──────────────────────────────┘ │  │
│  │                                 │                                 │  │
│  │  ┌──────────────────────────────▼──────────────────────────────┐ │  │
│  │  │           OTLP Exporter (every 10 seconds)                   │ │  │
│  │  │           - Batches metrics                                  │ │  │
│  │  │           - Protobuf serialization                           │ │  │
│  │  │           - gRPC to localhost:4317                           │ │  │
│  │  └──────────────────────────────┬──────────────────────────────┘ │  │
│  └─────────────────────────────────┼─────────────────────────────────┘  │
└────────────────────────────────────┼────────────────────────────────────┘
                                     │
                                     │ gRPC (OTLP Protocol)
                                     │ Port: 4317
                                     │
                    ┌────────────────▼────────────────┐
                    │                                  │
                    │   Docker: toygres-otel-collector │
                    │   (OpenTelemetry Collector)      │
                    │                                  │
                    │   Receives: Metrics              │
                    │   Processes: Batching            │
                    │   Exports: Prometheus format     │
                    │                                  │
                    └────────────────┬────────────────┘
                                     │
                                     │ HTTP (Prometheus scrape)
                                     │ Port: 8889
                                     │ Format: Prometheus metrics
                                     │
                    ┌────────────────▼────────────────┐
                    │                                  │
                    │   Docker: toygres-prometheus     │
                    │   (Time-Series Database)         │
                    │                                  │
                    │   Scrapes: Every 15 seconds      │
                    │   Stores: Metrics with labels    │
                    │   Retention: Default unlimited   │
                    │                                  │
                    └────────────────┬────────────────┘
                                     │
                                     │ HTTP (PromQL API)
                                     │ Port: 9090
                                     │
                    ┌────────────────▼────────────────┐
                    │                                  │
                    │   Docker: toygres-grafana        │
                    │   (Visualization)                │
                    │                                  │
                    │   Queries: Prometheus           │
                    │   Displays: Dashboards          │
                    │   Access: localhost:3001        │
                    │                                  │
                    └──────────────────────────────────┘
                                     │
                                     │ HTTPS
                                     │ Port: 3001
                                     │
                          ┌──────────▼──────────┐
                          │   Your Browser      │
                          │   - Dashboards      │
                          │   - Alerts          │
                          │   - Queries         │
                          └─────────────────────┘
```

---

## Detailed Metrics Flow

### Phase 1: Instrumentation (In Duroxide Runtime)

```
┌─────────────────────────────────────────────────────────────────┐
│  Duroxide Runtime - Metrics Instrumentation                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  When Activity Executes:                                         │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  1. Activity starts                                       │   │
│  │     → metrics.activity_executions.inc()                  │   │
│  │       Labels: {activity_name, outcome, retry_attempt}    │   │
│  │                                                            │   │
│  │  2. Record start time                                     │   │
│  │     let start = Instant::now();                          │   │
│  │                                                            │   │
│  │  3. Execute activity                                      │   │
│  │     let result = run_activity().await;                   │   │
│  │                                                            │   │
│  │  4. Record duration                                       │   │
│  │     let duration = start.elapsed();                      │   │
│  │     metrics.activity_duration.record(duration.as_secs_f64()) │
│  │       Labels: {activity_name, outcome}                   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                   │
│  When Orchestration Lifecycle Events:                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  - Start      → orchestration_starts.inc()               │   │
│  │                   Labels: {name, version, initiated_by}  │   │
│  │                                                            │   │
│  │  - Complete   → orchestration_completions.inc()          │   │
│  │                   Labels: {name, status, turn_count}     │   │
│  │                                                            │   │
│  │  - Fail       → orchestration_failures.inc()             │   │
│  │                   Labels: {name, error_type}             │   │
│  │                                                            │   │
│  │  - ContinueAsNew → continue_as_new.inc()                 │   │
│  │                     Labels: {name, execution_id}         │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                   │
│  All metrics buffered in-memory (OpenTelemetry SDK)             │
│  Exported every 10 seconds via OTLP                             │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 2: Export (OTLP)

```
┌──────────────────────────────────────────────────────────┐
│  OpenTelemetry Exporter (in Toygres Process)              │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  Every 10 seconds:                                        │
│                                                            │
│  ┌────────────────────────────────────────────────────┐  │
│  │  1. Collect all metrics from SDK                   │  │
│  │     - 3 orchestration starts                        │  │
│  │     - 2 activity executions                         │  │
│  │     - 1 histogram update                            │  │
│  │                                                      │  │
│  │  2. Convert to OTLP protobuf format                │  │
│  │     Resource {                                      │  │
│  │       service.name: "toygres"                       │  │
│  │       service.version: "0.1.0"                      │  │
│  │     }                                                │  │
│  │     Metrics [                                       │  │
│  │       {name: "duroxide_activity_executions_total",  │  │
│  │        labels: {activity_name, outcome},            │  │
│  │        value: 2}                                    │  │
│  │     ]                                                │  │
│  │                                                      │  │
│  │  3. Send via gRPC                                   │  │
│  │     POST grpc://localhost:4317/v1/metrics          │  │
│  │     (OTLP Protocol)                                 │  │
│  └────────────────────────────────────────────────────┘  │
│                                                            │
│  Network: Host → Docker bridge → Container                │
└──────────────────────────────────────────────────────────┘
                          │
                          │
                          ▼
```

### Phase 3: Collection & Transform (OTLP Collector)

```
┌────────────────────────────────────────────────────────────────┐
│  OpenTelemetry Collector (Docker Container)                     │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌───────────────┐      ┌───────────────┐      ┌────────────┐ │
│  │   Receivers   │  →   │  Processors   │  →   │ Exporters  │ │
│  └───────────────┘      └───────────────┘      └────────────┘ │
│         │                       │                      │        │
│         │                       │                      │        │
│  ┌──────▼──────┐         ┌─────▼─────┐        ┌───────▼─────┐ │
│  │ OTLP gRPC   │         │  Batch    │        │ Prometheus  │ │
│  │ Port: 4317  │         │  - 10s    │        │ Exporter    │ │
│  │             │         │  - 1024   │        │ Port: 8889  │ │
│  │ Receives:   │         │   items   │        │             │ │
│  │ - Metrics   │         │           │        │ Converts:   │ │
│  │ - Logs      │         │ Aggregates │        │ OTLP →      │ │
│  │ - Traces    │         │ Enriches  │        │ Prometheus  │ │
│  │ (protobuf)  │         │           │        │ format      │ │
│  └─────────────┘         └───────────┘        └─────────────┘ │
│                                                      │           │
│                                          Exposes HTTP endpoint  │
│                                          /metrics (Prometheus)  │
└──────────────────────────────────────────────────────┬──────────┘
                                                       │
                                                       │
                                                       ▼
```

### Phase 4: Storage (Prometheus)

```
┌────────────────────────────────────────────────────────────────┐
│  Prometheus (Docker Container)                                  │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Scraper (every 15 seconds)                              │  │
│  │                                                            │  │
│  │  GET http://otel-collector:8889/metrics                  │  │
│  │                                                            │  │
│  │  Receives Prometheus format:                             │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │ duroxide_activity_executions_total{                 │ │  │
│  │  │   activity_name="test-connection",                  │ │  │
│  │  │   outcome="success",                                 │ │  │
│  │  │   retry_attempt="0",                                 │ │  │
│  │  │   service_name="toygres"                            │ │  │
│  │  │ } 8                                                  │ │  │
│  │  │                                                       │ │  │
│  │  │ duroxide_activity_duration_seconds_bucket{          │ │  │
│  │  │   activity_name="test-connection",                  │ │  │
│  │  │   outcome="success",                                 │ │  │
│  │  │   le="0.5"                                          │ │  │
│  │  │ } 2                                                  │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Time-Series Database (TSDB)                            │  │
│  │                                                            │  │
│  │  Stores: Metric name + labels + timestamp + value       │  │
│  │                                                            │  │
│  │  Example data point:                                     │  │
│  │  {                                                        │  │
│  │    metric: "duroxide_activity_executions_total",        │  │
│  │    labels: {activity_name: "test-connection",           │  │
│  │             outcome: "success",                          │  │
│  │             retry_attempt: "0"},                         │  │
│  │    timestamp: 1763841501,                               │  │
│  │    value: 8                                              │  │
│  │  }                                                        │  │
│  │                                                            │  │
│  │  Indexed by: label combinations for fast queries        │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  Exposes: PromQL API on port 9090                               │
└────────────────────────────────────────────────────────────────┘
                                 │
                                 │
                                 ▼
```

### Phase 5: Visualization (Grafana)

```
┌────────────────────────────────────────────────────────────────┐
│  Grafana (Docker Container)                                     │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Datasource: Prometheus (http://prometheus:9090)        │  │
│  └──────────────────────────────────────────────────────────┘  │
│                          │                                      │
│  ┌───────────────────────▼──────────────────────────────────┐  │
│  │  Dashboard Panels                                         │  │
│  │                                                            │  │
│  │  Panel 1: "Activity Duration p95"                        │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │ Query (PromQL):                                     │ │  │
│  │  │   histogram_quantile(0.95,                          │ │  │
│  │  │     rate(duroxide_activity_duration_seconds_bucket  │ │  │
│  │  │     [5m])                                            │ │  │
│  │  │   ) by (activity_name)                              │ │  │
│  │  │                                                       │ │  │
│  │  │ Grafana sends HTTP GET to Prometheus:               │ │  │
│  │  │   /api/v1/query?query=histogram_quantile(...)       │ │  │
│  │  │                                                       │ │  │
│  │  │ Prometheus returns:                                  │ │  │
│  │  │   {                                                  │ │  │
│  │  │     result: [                                        │ │  │
│  │  │       {metric: {activity_name: "test-connection"},  │ │  │
│  │  │        value: [timestamp, "0.382"]}                  │ │  │
│  │  │     ]                                                │ │  │
│  │  │   }                                                  │ │  │
│  │  │                                                       │ │  │
│  │  │ Grafana renders: Line graph showing 0.382s         │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │                                                            │  │
│  │  Panel 2: "Orchestration Success Rate"                   │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │ Query:                                               │ │  │
│  │  │   sum(rate(completions{status="success"}[5m]))     │ │  │
│  │  │   /                                                  │ │  │
│  │  │   sum(rate(completions[5m]))                        │ │  │
│  │  │                                                       │ │  │
│  │  │ Renders: Gauge showing 96.4%                        │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │                                                            │  │
│  │  Auto-refresh: Every 10 seconds                          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  Access: http://localhost:3001 (admin/admin)                   │
└────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Timeline

```
T=0s     Activity executes in duroxide
         └─> Metric counter incremented in-memory
         
T=10s    OTLP exporter batch timer fires
         └─> Metrics sent to OTLP Collector via gRPC
         
T=15s    Prometheus scrape interval
         └─> Prometheus pulls metrics from OTLP Collector
         
T=20s    Grafana dashboard auto-refresh (10s interval)
         └─> Grafana queries Prometheus
         └─> Dashboard updates in browser
         
Total latency: 0-20 seconds from event to visualization
```

---

## Metric Labels - Example Breakdown

### Activity Execution Metric:

```
Metric Name: duroxide_activity_executions_total
Type: Counter
Current Value: 847

Labels (Dimensions):
├─ activity_name: "toygres-orchestrations::activity::cms-record-health-check"
├─ outcome: "success"
├─ retry_attempt: "0"
├─ job: "toygres" (added by Prometheus)
├─ service_name: "toygres" (added by OTLP)
└─ service_version: "0.1.0" (added by OTLP)

This creates a unique time series:
  duroxide_activity_executions_total{
    activity_name="cms-record-health-check",
    outcome="success",
    retry_attempt="0",
    service_name="toygres"
  }

Prometheus stores:
  [timestamp1, value1]
  [timestamp2, value2]
  [timestamp3, value3]
  ...
```

### Histogram Metric:

```
Metric Name: duroxide_activity_duration_seconds
Type: Histogram

Becomes multiple time series in Prometheus:
├─ duroxide_activity_duration_seconds_bucket{le="0.01", activity_name="...", outcome="..."} = 0
├─ duroxide_activity_duration_seconds_bucket{le="0.05", activity_name="...", outcome="..."} = 0
├─ duroxide_activity_duration_seconds_bucket{le="0.1", activity_name="...", outcome="..."} = 0
├─ duroxide_activity_duration_seconds_bucket{le="0.5", activity_name="...", outcome="..."} = 2
├─ duroxide_activity_duration_seconds_bucket{le="1", activity_name="...", outcome="..."} = 6
├─ duroxide_activity_duration_seconds_bucket{le="2", activity_name="...", outcome="..."} = 6
├─ duroxide_activity_duration_seconds_bucket{le="+Inf", activity_name="...", outcome="..."} = 6
├─ duroxide_activity_duration_seconds_sum{activity_name="...", outcome="..."} = 2.3
└─ duroxide_activity_duration_seconds_count{activity_name="...", outcome="..."} = 6

Prometheus calculates percentiles:
  histogram_quantile(0.95, ...) = 0.8 seconds (95th percentile)
```

---

## Component Details

### 1. Toygres Server (Host Process)

```
Process: toygres-server
Location: Host machine (macOS)
Port: 8080 (API)

Responsibilities:
├─ Run duroxide runtime
├─ Execute orchestrations & activities
├─ Collect metrics via OpenTelemetry SDK
├─ Export metrics via OTLP (gRPC)
└─ Write logs to stdout/file

Configuration:
└─ OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
```

### 2. OTLP Collector (Docker Container)

```
Container: toygres-otel-collector
Image: otel/opentelemetry-collector:latest
Network: toygres_toygres-monitoring

Ports:
├─ 4317: OTLP gRPC receiver (from toygres)
├─ 4318: OTLP HTTP receiver (unused)
├─ 8889: Prometheus metrics exporter (to prometheus)
└─ 13133: Health check

Configuration: observability/otel-collector-config.yaml

Pipelines:
├─ Metrics: OTLP → Batch → Prometheus + Debug
└─ Logs: OTLP → Batch → Loki (ready, but duroxide doesn't send yet)
```

### 3. Prometheus (Docker Container)

```
Container: toygres-prometheus
Image: prom/prometheus:latest
Network: toygres_toygres-monitoring

Port: 9090 (Web UI & API)
Storage: Docker volume (prometheus-data)

Configuration: observability/prometheus.yml

Scrape Targets:
├─ otel-collector:8889 (every 15s)
└─ prometheus:9090 (self, every 15s)

Stores:
└─ ~9 metric types
   └─ ~200+ unique time series (different label combinations)
   └─ Retention: Unlimited (default)
```

### 4. Grafana (Docker Container)

```
Container: toygres-grafana
Image: grafana/grafana:latest
Network: toygres_toygres-monitoring

Port: 3001 (Web UI)
Storage: Docker volume (grafana-data)

Datasources:
├─ Prometheus (http://prometheus:9090)
└─ Loki (http://loki:3100)

Dashboards (auto-loaded):
├─ toygres-production.json          (✅ Working)
├─ toygres-simple.json              (✅ Working)
├─ toygres-logs.json                (✅ Working)
└─ toygres-active-orchestrations.json (⚠️ Needs duroxide gauge)
```

---

## Network Flow

```
Host Machine (macOS)
├─ toygres-server process
│  └─ Sends to: localhost:4317
│     └─ Docker port mapping: host:4317 → container:4317
│
└─ Docker Network: toygres_toygres-monitoring
   ├─ toygres-otel-collector (4317, 8889)
   │  └─ Exposes: otel-collector:8889 (in Docker network)
   │
   ├─ toygres-prometheus (9090)
   │  └─ Scrapes: http://otel-collector:8889/metrics
   │
   ├─ toygres-loki (3100)
   │  └─ Receives: (Ready for OTLP logs)
   │
   └─ toygres-grafana (3001)
      ├─ Queries: http://prometheus:9090
      ├─ Queries: http://loki:3100
      └─ Exposes: localhost:3001 (port mapped to host)
```

---

## Metrics Lifecycle - Concrete Example

### Example: Track "DeployPostgres" Activity

```
Step 1: Activity Executes
┌──────────────────────────────────────┐
│ Rust Code:                           │
│                                       │
│ let result = deploy_postgres().await;│
│                                       │
│ Duroxide Runtime:                    │
│ - Start time: T0                     │
│ - Execute activity                   │
│ - End time: T1 (duration: 4.2s)     │
│ - Outcome: Success                   │
└──────────────────────────────────────┘
                 │
                 ▼
Step 2: Record Metrics (Instant)
┌──────────────────────────────────────┐
│ OpenTelemetry SDK:                   │
│                                       │
│ activity_executions.inc()            │
│   {activity_name: "DeployPostgres",  │
│    outcome: "success",                │
│    retry_attempt: "0"}                │
│                                       │
│ activity_duration.record(4.2)        │
│   {activity_name: "DeployPostgres",  │
│    outcome: "success"}                │
│   Bucket: le="5" → increment         │
│                                       │
│ Stored in memory buffer              │
└──────────────────────────────────────┘
                 │
                 ▼ (waits up to 10s)
Step 3: Batch Export
┌──────────────────────────────────────┐
│ OTLP Exporter:                       │
│                                       │
│ Timer fires (T0 + 10s)               │
│ Collect all pending metrics          │
│ Serialize to protobuf                │
│ gRPC call to localhost:4317          │
└──────────────────────────────────────┘
                 │
                 ▼
Step 4: OTLP Collector Receives
┌──────────────────────────────────────┐
│ OTLP Collector:                      │
│                                       │
│ Receives gRPC request                │
│ Batch processor accumulates          │
│ Converts to Prometheus format        │
│ Exposes on :8889/metrics             │
└──────────────────────────────────────┘
                 │
                 ▼ (waits up to 15s)
Step 5: Prometheus Scrapes
┌──────────────────────────────────────┐
│ Prometheus:                          │
│                                       │
│ Scrape interval fires (every 15s)    │
│ GET otel-collector:8889/metrics      │
│ Parse Prometheus format              │
│ Store in TSDB with timestamp         │
└──────────────────────────────────────┘
                 │
                 ▼
Step 6: Grafana Queries
┌──────────────────────────────────────┐
│ Grafana:                             │
│                                       │
│ Dashboard auto-refresh (10s)         │
│ Execute PromQL query:                │
│   histogram_quantile(0.95,           │
│     rate(activity_duration[5m])      │
│   )                                   │
│                                       │
│ Prometheus returns: 4.1 seconds      │
│ Grafana renders: Line graph          │
└──────────────────────────────────────┘
                 │
                 ▼
Step 7: User Sees Data
┌──────────────────────────────────────┐
│ Browser:                             │
│                                       │
│ Dashboard shows:                     │
│ "DeployPostgres p95 duration: 4.1s" │
│                                       │
│ Updated automatically every 10s      │
└──────────────────────────────────────┘

Total time from event to visualization: 10s (export) + 15s (scrape) + 10s (refresh) = ~35s max
Best case: 10s + 0s (just scraped) + 0s (just refreshed) = ~10s
```

---

## Cardinality Example

### Current Toygres Metrics

```
Activity executions with labels:
- 6 different activity names
- 2 outcomes (success, app_error)
- 2 retry attempts tracked (0, 1)
= 6 × 2 × 2 = 24 time series

Activity duration histogram:
- 6 activity names
- 2 outcomes
- 13 buckets per histogram
= 6 × 2 × 13 = 156 time series

Orchestration starts:
- 1 orchestration type (instance-actor)
- 2 initiated_by values (client, continueAsNew)
= 1 × 2 = 2 time series

Total: ~200 time series (excellent, very manageable!)
```

---

## What's Happening Right Now

```
Live System State:
┌─────────────────────────────────────────────────────────────┐
│                                                               │
│  Toygres Server (PID: 23657)                                │
│    │                                                          │
│    ├─ Running ~10 instance actors (continuous orchestrations)│
│    ├─ Each performs health checks every 30 seconds          │
│    ├─ Executes 3 activities per health check:               │
│    │  1. GetInstanceConnection                              │
│    │  2. TestConnection                                      │
│    │  3. RecordHealthCheck                                   │
│    │  4. UpdateInstanceHealth                                │
│    │                                                          │
│    └─ Every 10 seconds: Export metrics →                    │
│                                                               │
│  OTLP Collector (toygres-otel-collector)                     │
│    └─ Receiving metrics every 10s                           │
│    └─ Exporting to Prometheus format on :8889               │
│                                                               │
│  Prometheus (toygres-prometheus)                             │
│    └─ Scraping :8889 every 15s                              │
│    └─ Storing ~200 time series                              │
│    └─ Queryable at http://localhost:9090                    │
│                                                               │
│  Grafana (toygres-grafana)                                   │
│    └─ Auto-refreshing dashboards every 10s                  │
│    └─ Displaying real-time metrics                          │
│    └─ Access: http://localhost:3001                         │
│                                                               │
└─────────────────────────────────────────────────────────────┘

Current Metrics Count:
  Activity executions: ~800+
  Orchestration starts: ~1100+
  Continue-as-new: ~100+
  
Dashboard showing:
  ✅ Activity durations (p50, p95, p99)
  ✅ Success/failure rates
  ✅ Execution rates
  ⚠️ Active count (incorrect, needs gauge from duroxide)
```

---

## Tech Stack Summary

| Layer | Technology | Purpose | Status |
|-------|-----------|---------|--------|
| **Instrumentation** | OpenTelemetry SDK | Collect metrics in app | ✅ |
| **Export Protocol** | OTLP (gRPC) | Send metrics to collector | ✅ |
| **Collection** | OTLP Collector | Receive & transform | ✅ |
| **Storage** | Prometheus | Time-series database | ✅ |
| **Visualization** | Grafana | Dashboards & queries | ✅ |
| **Log Aggregation** | Loki | Log storage & search | ✅ |
| **Query Language** | PromQL / LogQL | Query metrics & logs | ✅ |

---

## Performance Characteristics

```
Duroxide Runtime Overhead:
├─ Metrics collection: ~2-3% CPU
├─ Memory for buffers: ~5MB
├─ Network: ~50KB/10s (OTLP export)
└─ Total: <5% overhead

OTLP Collector:
├─ CPU: <1% idle, ~10% during scrape
├─ Memory: ~50MB
└─ Network: 50KB/10s in, 100KB/15s out

Prometheus:
├─ CPU: ~5% during scrape
├─ Memory: ~200MB (for ~200 series, 15-day retention)
├─ Disk: ~10MB/day
└─ Query latency: <100ms

Grafana:
├─ CPU: ~1% idle, ~20% during dashboard load
├─ Memory: ~150MB
└─ Dashboard render: <1s
```

---

## Configuration Sources

```
Toygres Configuration:
└─ observability/env.local.example
   ├─ DUROXIDE_OBSERVABILITY_ENABLED=true
   ├─ OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
   ├─ DUROXIDE_LOG_FORMAT=json
   └─ DUROXIDE_LOG_LEVEL=info

OTLP Collector Configuration:
└─ observability/otel-collector-config.yaml
   ├─ Receivers: OTLP (gRPC, HTTP)
   ├─ Processors: Batch (10s, 1024 items)
   ├─ Exporters: Prometheus (:8889), Loki (OTLP), Debug
   └─ Pipelines: Metrics, Logs

Prometheus Configuration:
└─ observability/prometheus.yml
   └─ Scrape: otel-collector:8889 every 15s

Grafana Configuration:
└─ observability/grafana/provisioning/
   ├─ Datasources: Prometheus, Loki (auto-configured)
   └─ Dashboards: All .json files auto-loaded
```

---

## Quick Reference: Where Is Everything?

```
Metrics Data:
  Source:       Duroxide runtime (in-memory)
  Transit:      OTLP gRPC → localhost:4317
  Storage:      Prometheus container (TSDB)
  Access:       Grafana dashboards or http://localhost:9090
  
Logs Data:
  Source:       Duroxide stdout
  Transit:      Script → HTTP → localhost:3100
  Storage:      Loki container
  Access:       Grafana logs dashboard or Explore
  
Configurations:
  Location:     /Users/affandar/workshop/toygres/observability/
  
Dashboards:
  Location:     observability/grafana/dashboards/*.json
  Auto-loaded:  On Grafana startup
  
Scripts:
  Location:     scripts/start-observability.sh, etc.
```

---

## This Diagram Answers:

✅ Where do metrics come from? → Duroxide runtime instrumentation  
✅ How do they get to Prometheus? → OTLP gRPC → Collector → HTTP scrape  
✅ What format are they in? → OpenTelemetry → Prometheus format  
✅ How are labels added? → At instrumentation time in duroxide  
✅ What's the latency? → ~10-35 seconds from event to dashboard  
✅ What's working? → Metrics, logs, dashboards (except active count)  
✅ What's broken? → Active orchestration tracking (needs gauge from duroxide)  

**Reference this diagram when explaining the system to others!** 📊


