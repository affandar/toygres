# Toygres Grafana Dashboards Guide

## Available Dashboards

### 1. **Toygres Active Orchestrations** ⭐ NEW!
**URL:** http://localhost:3001/d/toygres-active/toygres-active-orchestrations

**Purpose:** Real-time view of currently running orchestrations

**Panels:**
1. **Total Active Orchestrations** (Gauge) - Overall active count (calculated: Starts - Completions)
2. **Total Starts / Successes / Failures** (Stats) - Cumulative counters
3. **Active Orchestrations by Type** (Pie Chart) - Breakdown by orchestration_name
4. **Orchestrations by Initiation Method** (Donut) - client vs continueAsNew vs suborchestration
5. **Active Orchestrations Over Time** (Time Series) - Stacked area chart showing count by type
6. **Active Count by Orchestration** (Bar Chart) - Horizontal bars for easy comparison
7. **Cumulative Starts by Initiation Method** (Time Series) - Growth trends
8. **Active Orchestrations Breakdown** (Table) - Full details with all labels
9. **Detailed Orchestration Stats** (Table) - Starts vs Completions, all labels visible

**Best For:**
- Monitoring instance actors (continuous orchestrations)
- Identifying stuck or leaked orchestrations
- Capacity planning
- Understanding orchestration composition

---

### 2. **Toygres Production Metrics**
**URL:** http://localhost:3001/d/toygres-production/toygres-production-metrics

**Purpose:** Performance and health monitoring

**Panels:**
1. **Orchestration Start Rate** - Throughput gauge
2. **Orchestration Success Rate** - Health indicator with thresholds
3. **Orchestration Failures** - Failure rate
4. **Continue-as-New Operations** - Instance actor operations
5. **Orchestration Start Rate by Type** - Breakdown with labels
6. **Orchestration Completion Rate by Status** - Success vs failed
7. **Activity Duration Percentiles** - p50/p95/p99 by activity_name
8. **Activity Execution Rate by Outcome** - Success vs errors
9. **Orchestration Failures by Type** - Error classification
10. **Orchestration History Size** - Event count tracking
11. **Orchestration Turns to Completion** - Performance indicator

**Best For:**
- SLA monitoring
- Performance optimization
- Error tracking
- Activity-level insights

---

### 3. **Toygres Simple (Working)**
**URL:** http://localhost:3001/d/toygres-simple/toygres-simple

**Purpose:** Basic metrics overview (legacy, before rich labels)

**Panels:**
- Activity Execution Rate
- Total Activity Executions
- Activity Errors
- Cumulative Executions
- Errors Over Time

**Best For:**
- Quick health check
- Simple monitoring
- Fallback if detailed metrics unavailable

---

### 4. **Toygres Logs**
**URL:** http://localhost:3001/d/toygres-logs/toygres-logs

**Purpose:** Log aggregation and search

**Panels:**
- Live Logs (All)
- Log Rate by Level
- Errors (last 5m)
- Error Logs Only
- Orchestration Logs
- Activity Logs
- Parsed JSON Errors

**Best For:**
- Debugging
- Error investigation
- Trace specific instance_id
- Log analysis

---

## Key Metrics Reference

### Calculate Active Orchestrations

**Total Active:**
```promql
sum(duroxide_orchestration_starts_total) 
- 
sum(duroxide_orchestration_completions_total)
```

**By Orchestration Name:**
```promql
duroxide_orchestration_starts_total 
- on(orchestration_name, version) 
duroxide_orchestration_completions_total
```

**By Initiation Method:**
```promql
duroxide_orchestration_starts_total{initiated_by="client"}
- on(orchestration_name) 
duroxide_orchestration_completions_total
```

### Available Labels

**Orchestration Metrics:**
- `orchestration_name` - Full qualified name
- `version` - Semantic version (e.g., "1.0.0")
- `initiated_by` - "client" | "continueAsNew" | "suborchestration" | "signal"
- `status` - "success" | "failed" | "continuedAsNew"
- `final_turn_count` - "1-5" | "6-10" | "11-50" | "50+"
- `error_type` - Classification of failure

**Activity Metrics:**
- `activity_name` - Full qualified name
- `outcome` - "success" | "app_error" | "infra_error" | "timeout"
- `retry_attempt` - "0" | "1" | "2" | "3+"

## Dashboard Recommendations

### For Operations/SRE:
1. Start with **"Active Orchestrations"** - See what's running
2. Check **"Production Metrics"** - Health and performance
3. Investigate issues in **"Logs"** - Detailed debugging

### For Development:
1. Use **"Production Metrics"** - Identify slow activities
2. Use **"Active Orchestrations"** table - Understand orchestration lifecycle
3. Use **"Logs"** - Debug specific instance_id

### For Executives:
1. **"Production Metrics"** → Success Rate gauge
2. **"Active Orchestrations"** → Total active gauge
3. Set up alerting rules

## Alert Rules (Recommended)

### High Active Count (Potential Leak)
```yaml
- alert: HighActiveOrchestrations
  expr: sum(duroxide_orchestration_starts_total) - sum(duroxide_orchestration_completions_total) > 1000
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "High number of active orchestrations"
```

### Orchestration Failure Rate
```yaml
- alert: OrchestrationFailureRate
  expr: |
    rate(duroxide_orchestration_completions_total{status="failed"}[5m]) 
    / 
    rate(duroxide_orchestration_completions_total[5m]) > 0.1
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "Orchestration failure rate > 10%"
```

### Stuck Orchestrations
```yaml
- alert: StuckOrchestrations
  expr: |
    (duroxide_orchestration_starts_total 
    - on(orchestration_name) 
    duroxide_orchestration_completions_total) > 100
  for: 30m
  labels:
    severity: warning
  annotations:
    summary: "{{ $labels.orchestration_name }} has {{ $value }} stuck instances"
```

## Customization

### Add Custom Panels

1. Edit dashboard in Grafana UI
2. Click **"Add"** → **"Visualization"**
3. Select Prometheus datasource
4. Enter PromQL query
5. Configure visualization
6. **Save**

### Export Dashboard

```bash
# From Grafana UI: Dashboard → Share → Export
# Save JSON to: observability/grafana/dashboards/
```

### Create Dashboard from Scratch

Use the existing dashboards as templates:
- `toygres-active-orchestrations.json` - Best for showing current state
- `toygres-production.json` - Best for time series analysis
- `toygres-logs.json` - Best for log panels

## Performance Tips

1. **Use recording rules** for complex queries:
```yaml
# prometheus-rules.yml
- record: toygres:active_orchestrations:count
  expr: sum(duroxide_orchestration_starts_total) - sum(duroxide_orchestration_completions_total)
```

2. **Limit time ranges** for large datasets
3. **Use instant queries** for tables (format: table, instant: true)
4. **Enable auto-refresh** for live monitoring (10s recommended)

## Troubleshooting Dashboards

### "No data" shown

1. **Check time range** - Top right, try "Last 15 minutes"
2. **Check auto-refresh** - Should show "10s"
3. **Verify datasource** - Should be "Prometheus" or "Loki"
4. **Check query** - Click panel title → Edit → Query tab

### Slow dashboard loading

1. Reduce time range (e.g., 1h instead of 24h)
2. Use recording rules for expensive queries
3. Disable unused panels
4. Increase refresh interval (30s instead of 10s)

### Wrong data displayed

1. Click **"Refresh"** button (circular arrow)
2. Hard refresh browser (Cmd+Shift+R)
3. Check if toygres-server is running and sending metrics
4. Verify Prometheus is scraping: http://localhost:9090/targets

## Quick Access URLs

- **Active Orchestrations:** http://localhost:3001/d/toygres-active
- **Production Metrics:** http://localhost:3001/d/toygres-production
- **Simple Metrics:** http://localhost:3001/d/toygres-simple
- **Logs:** http://localhost:3001/d/toygres-logs
- **Prometheus:** http://localhost:9090
- **Loki:** http://localhost:3100

---

**Your active orchestrations dashboard is ready!** 

Go to: http://localhost:3001/d/toygres-active

You should see **~1000 active orchestrations** (your instance actors)! 📊


