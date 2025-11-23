# Toygres Observability - Current Status

**Last Updated:** 2025-11-22

## ✅ What's Working

### Metrics (OpenTelemetry → Prometheus → Grafana)

**Infrastructure:**
- ✅ OTLP Collector running (port 4317)
- ✅ Prometheus scraping metrics (port 9090)
- ✅ Grafana displaying dashboards (port 3001)
- ✅ Duroxide exporting metrics with `observability` feature enabled

**Metrics Available:**
- ✅ `duroxide_activity_executions_total` - WITH LABELS (activity_name, outcome, retry_attempt)
- ✅ `duroxide_activity_duration_seconds` - Histogram WITH LABELS (activity_name, outcome)
- ✅ `duroxide_orchestration_starts_total` - WITH LABELS (orchestration_name, version, initiated_by)
- ✅ `duroxide_orchestration_completions_total` - WITH LABELS (status, final_turn_count)
- ✅ `duroxide_orchestration_failures_total` - WITH LABELS (error_type)
- ✅ `duroxide_orchestration_duration_seconds` - Histogram WITH LABELS
- ✅ `duroxide_orchestration_history_size` - Histogram (event count)
- ✅ `duroxide_orchestration_turns` - Histogram (turn count)
- ✅ `duroxide_orchestration_continue_as_new_total` - Continue-as-new tracking

**Dashboards:**
- ✅ **Toygres Production Metrics** - Performance, success rates, durations
- ✅ **Toygres Simple** - Basic overview
- ✅ **Toygres Logs** - Log aggregation and filtering

### Logs (Loki)

**Infrastructure:**
- ✅ Loki running (port 3100)
- ✅ Log forwarder script pushing logs
- ✅ Logs viewable in Grafana Explore
- ✅ Logs searchable via LogQL

**What's Flowing:**
- ✅ Toygres server logs
- ✅ Duroxide orchestration logs
- ✅ Activity execution logs
- ✅ Structured fields (instance_id, orchestration_name, activity_name, etc.)

---

## ⚠️ Known Issues / Limitations

### 1. Active Orchestrations Count (CRITICAL)

**Issue:** Cannot accurately track how many orchestrations are currently running

**Why:** 
- The calculation `starts_total - completions_total` doesn't work
- Both are cumulative counters
- Continue-as-new counts as completion but orchestration is still active
- Result shows ~1000 active when reality is likely ~10-50

**Dashboard Affected:**
- ❌ "Toygres Active Orchestrations" dashboard exists but shows incorrect data

**Solution Needed:**
- Duroxide must provide `duroxide_active_orchestrations` gauge metric
- See: `docs/duroxide-active-orchestrations-metric-spec.md` for full spec

**Workaround:**
- Query CMS database directly for instance count
- Use toygres API: `curl http://localhost:8080/api/instances | jq '. | length'`

### 2. OTLP Log Export (Medium Priority)

**Issue:** Logs are not exported via OpenTelemetry

**Current State:**
- Logs go to stdout only
- Require external log forwarder script
- Not as seamless as metrics

**Solution Needed:**
- Duroxide should export logs via OTLP (same endpoint as metrics)
- See: `docs/duroxide-telemetry-spec.md` Section 4.1 for spec

**Workaround:**
- Using `scripts/push-logs-to-loki.sh` (works, but not ideal)
- Logs still queryable in Grafana

### 3. Missing Provider/Database Metrics

**Issue:** No visibility into database operation performance

**Metrics Needed:**
- `duroxide_provider_operation_duration_seconds` (histogram)
- `duroxide_provider_errors_total` (counter)
- `duroxide_provider_connection_pool_size` (gauge)

**Impact:**
- Cannot diagnose database performance issues
- Cannot correlate slow orchestrations with slow database

**Status:** Documented in main telemetry spec

---

## 📋 Duroxide Improvement Roadmap

Priority-ordered list for duroxide framework:

### Phase 1: Critical Metrics (BLOCKING)
1. **`duroxide_active_orchestrations` gauge** 🔴
   - Spec: `docs/duroxide-active-orchestrations-metric-spec.md`
   - Impact: Enables basic production monitoring
   - Effort: Medium (1-2 days)

### Phase 2: Observability Completeness
2. **OTLP log export** 🟡
   - Spec: `docs/duroxide-telemetry-spec.md` Section 4.1
   - Impact: Unified observability, no external shippers needed
   - Effort: Medium (2-3 days)

3. **Provider/database metrics** 🟡
   - Spec: `docs/duroxide-telemetry-spec.md` Section 1.3
   - Impact: Database performance visibility
   - Effort: Small (1 day)

### Phase 3: Advanced Features
4. **Worker queue depth metrics** 🟢
5. **Sub-orchestration tracking** 🟢
6. **Resource utilization metrics** 🟢

---

## 🎯 What Administrators Can Do Today

### Monitor Performance ✅
```promql
# Activity duration by name
histogram_quantile(0.95, 
  rate(duroxide_activity_duration_seconds_bucket[5m])
) by (activity_name)
```

### Track Success Rates ✅
```promql
# Orchestration success rate
sum(rate(duroxide_orchestration_completions_total{status="success"}[5m]))
/
sum(rate(duroxide_orchestration_completions_total[5m]))
```

### Identify Slow Activities ✅
```promql
# p99 duration per activity
histogram_quantile(0.99,
  rate(duroxide_activity_duration_seconds_bucket[5m])
) by (activity_name)
```

### Track Errors ✅
```promql
# Failures by error type
rate(duroxide_orchestration_failures_total[5m]) 
by (orchestration_name, error_type)
```

### Debug with Logs ✅
```logql
# All logs for specific instance
{job="toygres"} |= "instance_id" |= "create-myinstance-pg"
```

### Track Continue-as-New ✅
```promql
# How many instance actors are cycling
sum(duroxide_orchestration_continue_as_new_total) by (orchestration_name)
```

---

## 🚫 What Administrators CANNOT Do (Yet)

### Monitor Active Orchestration Count ❌
**Query:**
```promql
# This doesn't work correctly:
sum(duroxide_orchestration_starts_total) 
- 
sum(duroxide_orchestration_completions_total)
```

**Workaround:**
```bash
# Query toygres CMS database
psql $DATABASE_URL -c "SELECT state, COUNT(*) FROM cms.instances GROUP BY state;"
```

### Set Alerts on Active Count ❌
Cannot alert on "too many active orchestrations" without accurate gauge.

### Capacity Planning on Active Load ❌
Cannot determine if we need more workers without accurate active count.

---

## Configuration Files

```
observability/
├── otel-collector-config.yaml     - OTLP → Prometheus + Loki (ready for logs)
├── prometheus.yml                  - Scraping config
├── loki-config.yaml               - Log storage
├── grafana/
│   ├── provisioning/
│   │   ├── datasources/           - Auto-loaded on Grafana startup
│   │   └── dashboards/            - Auto-loaded on Grafana startup
│   └── dashboards/
│       ├── toygres-production.json         ✅ Working
│       ├── toygres-simple.json            ✅ Working
│       ├── toygres-logs.json              ✅ Working
│       └── toygres-active-orchestrations.json  ⚠️ Needs duroxide gauge metric
├── env.local.example              - Local dev environment
└── env.aks.example                - AKS/production environment
```

## Scripts

```
scripts/
├── start-observability.sh         ✅ Working - Starts Docker stack
├── stop-observability.sh          ✅ Working - Stops stack
├── observability-status.sh        ✅ Working - Health checks
├── start-control-plane.sh         ✅ Working - Full startup (with logs)
├── stop-control-plane.sh          ✅ Working - Full shutdown
├── force-kill-all.sh              ✅ Working - Emergency cleanup
└── push-logs-to-loki.sh          ✅ Working - Log forwarding (auto-started)
```

## Documentation

```
docs/
├── observability-quickstart.md                   - 5-minute setup guide
├── control-plane-guide.md                        - Usage guide
├── duroxide-telemetry-spec.md                    - Complete framework spec
└── duroxide-active-orchestrations-metric-spec.md - Focused active metric spec

observability/
├── README.md                                     - Full reference
└── DASHBOARDS.md                                 - Dashboard guide
```

---

## Next Steps

### For Toygres (You):
1. ✅ Use existing dashboards for monitoring
2. ✅ Track performance with activity duration metrics
3. ✅ Monitor errors and success rates
4. ⚠️ Use CMS database as workaround for active instance count

### For Duroxide (Also You):
1. 🔴 Implement `duroxide_active_orchestrations` gauge (see spec)
2. 🟡 Implement OTLP log export
3. 🟡 Add provider/database metrics
4. 🟢 Add worker queue metrics

---

## Metrics Quality: Before vs After Update

### Before (Commit d426cb5):
```
duroxide_activity_executions_total = 100
❌ No labels
❌ Can't tell which activities
❌ Can't tell success vs failure
```

### After (Commit 0077f60):
```
duroxide_activity_executions_total{
  activity_name="toygres::DeployPostgres",
  outcome="success",
  retry_attempt="0"
} = 85

duroxide_activity_executions_total{
  activity_name="toygres::DeployPostgres",
  outcome="app_error",
  retry_attempt="1"
} = 5

✅ Full labels
✅ Multi-dimensional queries
✅ Actionable insights
✅ Production-ready
```

---

## Status Summary

| Feature | Status | Notes |
|---------|--------|-------|
| Metrics Export | ✅ Working | Rich labels, histograms |
| Metrics Dashboards | ✅ Working | 3 dashboards functional |
| Log Export | ⚠️ Workaround | Script-based, works but not OTLP |
| Log Dashboards | ✅ Working | Searchable in Grafana |
| Active Count Tracking | ❌ Broken | Need gauge metric from duroxide |
| Provider Metrics | ❌ Missing | Database performance invisible |
| Alerting | 🟡 Possible | Can set up, but limited without active count |

**Overall:** 80% operational, 20% blocked on duroxide framework features

---

**Bottom Line:** Observability is working well for performance monitoring and debugging. The critical gap is accurate active orchestration tracking, which requires duroxide framework changes.

See `docs/duroxide-active-orchestrations-metric-spec.md` for the complete implementation guide! 🎯


