# Duroxide Metrics: Available vs Implemented in Toygres Dashboards

## Metrics Available (from Duroxide Observability Guide)

Based on the [duroxide observability guide](https://github.com/affandar/duroxide/blob/main/docs/observability-guide.md):

### Orchestration Metrics

| Metric Name | Type | Labels | Description | Status in Toygres |
|-------------|------|--------|-------------|-------------------|
| `duroxide_orchestration_starts_total` | Counter | orchestration_name, version, initiated_by | Orchestrations started | ✅ **IN DASHBOARD** |
| `duroxide_orchestration_completions_total` | Counter | orchestration_name, version, status, final_turn_count | Successful completions | ✅ **IN DASHBOARD** |
| `duroxide_orchestration_failures_total` | Counter | orchestration_name, error_type | Failures by error type | ✅ **IN DASHBOARD** |
| `duroxide_orchestration_infrastructure_errors_total` | Counter | orchestration_name, operation, error_type | Infra errors | ❌ NOT IN DASHBOARD |
| `duroxide_orchestration_configuration_errors_total` | Counter | orchestration_name, error_kind | Config errors | ❌ NOT IN DASHBOARD |
| `duroxide_orchestration_history_size` | Histogram | orchestration_name | Event count at completion | ✅ **IN DASHBOARD** |
| `duroxide_orchestration_turns` | Histogram | orchestration_name | Turns to completion | ✅ **IN DASHBOARD** |
| `duroxide_orchestration_duration_seconds` | Histogram | orchestration_name, status | End-to-end duration | ✅ **IN DASHBOARD** |
| `duroxide_orchestration_continue_as_new_total` | Counter | orchestration_name, execution_id | Continue-as-new ops | ✅ **IN DASHBOARD** |

**Orchestration Metrics in Dashboards: 6/9 (67%)**

---

### Activity Metrics

| Metric Name | Type | Labels | Description | Status in Toygres |
|-------------|------|--------|-------------|-------------------|
| `duroxide_activity_executions_total` | Counter | activity_name, outcome, retry_attempt | Execution outcomes | ✅ **IN DASHBOARD** |
| `duroxide_activity_duration_seconds` | Histogram | activity_name, outcome | Execution duration | ✅ **IN DASHBOARD** |
| `duroxide_activity_app_errors_total` | Counter | activity_name | Application errors | ⚠️ EXISTS, NOT VISUALIZED |
| `duroxide_activity_infrastructure_errors_total` | Counter | activity_name, operation | Infrastructure errors | ❌ NOT IN DASHBOARD |
| `duroxide_activity_configuration_errors_total` | Counter | activity_name, error_kind | Configuration errors | ❌ NOT IN DASHBOARD |

**Activity Metrics in Dashboards: 2/5 (40%)**

---

### Provider Metrics

| Metric Name | Type | Labels | Description | Status in Toygres |
|-------------|------|--------|-------------|-------------------|
| `duroxide_provider_fetch_orchestration_item_duration_ms` | Histogram | N/A | Fetch latency | ❌ NOT IN DASHBOARD |
| `duroxide_provider_ack_orchestration_item_duration_ms` | Histogram | N/A | Ack latency | ❌ NOT IN DASHBOARD |
| `duroxide_provider_ack_worker_duration_ms` | Histogram | N/A | Worker ack latency | ❌ NOT IN DASHBOARD |
| `duroxide_provider_ack_orchestration_retries_total` | Counter | N/A | Retry attempts | ❌ NOT IN DASHBOARD |
| `duroxide_provider_infrastructure_errors_total` | Counter | operation, error_type | Provider errors | ❌ NOT IN DASHBOARD |

**Provider Metrics in Dashboards: 0/5 (0%)**

---

### Client Metrics

| Metric Name | Type | Labels | Description | Status in Toygres |
|-------------|------|--------|-------------|-------------------|
| `duroxide_client_orchestration_starts_total` | Counter | N/A | Orchestrations started via client | ❌ NOT IN DASHBOARD |
| `duroxide_client_external_events_raised_total` | Counter | N/A | Events raised | ❌ NOT IN DASHBOARD |
| `duroxide_client_cancellations_total` | Counter | N/A | Cancellations requested | ❌ NOT IN DASHBOARD |
| `duroxide_client_wait_duration_ms` | Histogram | N/A | Wait operation duration | ❌ NOT IN DASHBOARD |

**Client Metrics in Dashboards: 0/4 (0%)**

---

## Summary Statistics

### Overall Coverage

| Category | Available Metrics | In Dashboards | Coverage |
|----------|-------------------|---------------|----------|
| **Orchestration** | 9 | 6 | 67% |
| **Activity** | 5 | 2 | 40% |
| **Provider** | 5 | 0 | 0% |
| **Client** | 4 | 0 | 0% |
| **TOTAL** | 23 | 8 | **35%** |

---

## Current Toygres Dashboards

### Dashboard 1: "Toygres Production Metrics"

**Metrics Used:**
1. ✅ `duroxide_orchestration_starts_total` - Start rate gauge
2. ✅ `duroxide_orchestration_completions_total` - Success rate, completion rate by status
3. ✅ `duroxide_orchestration_failures_total` - Failures gauge, failures by type
4. ✅ `duroxide_orchestration_continue_as_new_total` - Continue-as-new gauge
5. ✅ `duroxide_activity_duration_seconds` - Activity duration percentiles
6. ✅ `duroxide_activity_executions_total` - Activity execution rate by outcome
7. ✅ `duroxide_orchestration_history_size` - History size percentiles
8. ✅ `duroxide_orchestration_turns` - Turns percentiles

**Missing from this dashboard:**
- Infrastructure error rates
- Configuration error tracking
- Provider/database performance metrics
- Client operation metrics

---

### Dashboard 2: "Toygres Active Orchestrations"

**Metrics Used:**
1. ✅ `duroxide_orchestration_starts_total` - For calculation attempts
2. ✅ `duroxide_orchestration_completions_total` - For calculation attempts
3. ✅ `duroxide_orchestration_continue_as_new_total` - Continue-as-new tracking

**Issues:**
- ❌ Uses incorrect calculation (starts - completions)
- ⚠️ Needs `duroxide_active_orchestrations` gauge (not yet available)

---

### Dashboard 3: "Toygres Simple"

**Metrics Used:**
1. ✅ `duroxide_activity_executions_total` - Execution rate, total count
2. ⚠️ `duroxide_activity_app_errors_total` - Errors (exists but not visualized well)
3. ✅ `duroxide_orchestration_failures_total` - Failures over time

---

### Dashboard 4: "Toygres Logs"

**Uses:** Loki (log queries), not metrics

---

## Metrics NOT Used But Available

### High-Value Missing Metrics

#### 1. Infrastructure Errors (CRITICAL)
```promql
# Orchestration infrastructure errors
duroxide_orchestration_infrastructure_errors_total{
  orchestration_name="...",
  operation="fetch|ack|save",
  error_type="..."
}

# Activity infrastructure errors  
duroxide_activity_infrastructure_errors_total{
  activity_name="...",
  operation="..."
}

# Provider infrastructure errors
duroxide_provider_infrastructure_errors_total{
  operation="fetch|ack|...",
  error_type="timeout|connection|deadlock|..."
}
```

**Why Important:**
- Distinguish infrastructure issues from application bugs
- Alert on database problems
- Track transient vs permanent errors

**Dashboard Panel Needed:**
```
Title: "Infrastructure Error Rate"
Query: rate(duroxide_*_infrastructure_errors_total[5m]) by (operation, error_type)
Type: Time series (stacked)
```

---

#### 2. Configuration Errors (HIGH)
```promql
duroxide_orchestration_configuration_errors_total{
  orchestration_name="...",
  error_kind="unregistered|missing_version|nondeterminism|..."
}

duroxide_activity_configuration_errors_total{
  activity_name="...",
  error_kind="..."
}
```

**Why Important:**
- Catch deployment issues early
- Identify nondeterminism bugs
- Track registration problems

**Dashboard Panel Needed:**
```
Title: "Configuration Errors (Deployment Issues)"
Query: duroxide_*_configuration_errors_total by (error_kind)
Type: Table
```

---

#### 3. Provider Performance (HIGH)
```promql
duroxide_provider_fetch_orchestration_item_duration_ms{le="..."}
duroxide_provider_ack_orchestration_item_duration_ms{le="..."}
duroxide_provider_ack_worker_duration_ms{le="..."}
```

**Why Important:**
- Database performance directly affects orchestration speed
- Identify slow queries
- Correlate with activity duration

**Dashboard Panel Needed:**
```
Title: "Database Operation Latency (p95)"
Query: histogram_quantile(0.95,
  rate(duroxide_provider_*_duration_ms_bucket[5m])
) by (operation)
Type: Time series
```

---

#### 4. Provider Retries (MEDIUM)
```promql
duroxide_provider_ack_orchestration_retries_total
```

**Why Important:**
- Track database contention
- Identify locking issues
- Alert on retry storms

---

#### 5. Client Operations (LOW)
```promql
duroxide_client_orchestration_starts_total
duroxide_client_external_events_raised_total
duroxide_client_cancellations_total
duroxide_client_wait_duration_ms
```

**Why Important:**
- Track API usage
- Monitor client-side performance
- Not critical for orchestration monitoring

---

## Recommended Dashboard Additions

### New Panel Set 1: "Infrastructure Health"

**For existing dashboards, add:**

#### Panel: Infrastructure Error Rate
```json
{
  "title": "Infrastructure Error Rate",
  "targets": [{
    "expr": "rate(duroxide_orchestration_infrastructure_errors_total[1m])",
    "legendFormat": "Orch - {{operation}}: {{error_type}}"
  }, {
    "expr": "rate(duroxide_activity_infrastructure_errors_total[1m])",
    "legendFormat": "Activity - {{operation}}"
  }, {
    "expr": "rate(duroxide_provider_infrastructure_errors_total[1m])",
    "legendFormat": "Provider - {{operation}}: {{error_type}}"
  }],
  "type": "timeseries"
}
```

#### Panel: Configuration Errors (Table)
```json
{
  "title": "Configuration Errors (Action Required)",
  "targets": [{
    "expr": "duroxide_orchestration_configuration_errors_total",
    "format": "table"
  }, {
    "expr": "duroxide_activity_configuration_errors_total",
    "format": "table"
  }],
  "type": "table"
}
```

### New Panel Set 2: "Database Performance"

#### Panel: Database Operation Latency (p95)
```json
{
  "title": "Database Operation Latency (p95)",
  "targets": [{
    "expr": "histogram_quantile(0.95, rate(duroxide_provider_fetch_orchestration_item_duration_ms_bucket[5m]))",
    "legendFormat": "Fetch (p95)"
  }, {
    "expr": "histogram_quantile(0.95, rate(duroxide_provider_ack_orchestration_item_duration_ms_bucket[5m]))",
    "legendFormat": "Ack (p95)"
  }],
  "type": "timeseries"
}
```

#### Panel: Provider Retry Rate
```json
{
  "title": "Database Retry Rate",
  "targets": [{
    "expr": "rate(duroxide_provider_ack_orchestration_retries_total[1m])"
  }],
  "type": "gauge"
}
```

---

## Priority: What to Add First

### Priority 1: Infrastructure Errors (CRITICAL)
**Why:** Distinguish app bugs from infrastructure problems  
**Queries:**
```promql
rate(duroxide_orchestration_infrastructure_errors_total[5m])
rate(duroxide_activity_infrastructure_errors_total[5m])
rate(duroxide_provider_infrastructure_errors_total[5m])
```

### Priority 2: Database Performance (HIGH)
**Why:** Database is often the bottleneck  
**Queries:**
```promql
histogram_quantile(0.95, rate(duroxide_provider_fetch_orchestration_item_duration_ms_bucket[5m]))
histogram_quantile(0.95, rate(duroxide_provider_ack_orchestration_item_duration_ms_bucket[5m]))
```

### Priority 3: Configuration Errors (HIGH)
**Why:** Catch deployment issues  
**Queries:**
```promql
duroxide_orchestration_configuration_errors_total
duroxide_activity_configuration_errors_total
```

### Priority 4: Provider Retries (MEDIUM)
**Why:** Database contention indicator  
**Queries:**
```promql
rate(duroxide_provider_ack_orchestration_retries_total[5m])
```

### Priority 5: Client Metrics (LOW)
**Why:** Nice to have, not critical  
**Queries:**
```promql
rate(duroxide_client_orchestration_starts_total[5m])
rate(duroxide_client_external_events_raised_total[5m])
```

---

## Metrics Exposed But Not in Dashboards

### Currently Available in Prometheus (When Running):

**Fully Utilized:**
1. ✅ `duroxide_activity_executions_total{activity_name, outcome, retry_attempt}`
2. ✅ `duroxide_activity_duration_seconds{activity_name, outcome}`
3. ✅ `duroxide_orchestration_starts_total{orchestration_name, version, initiated_by}`
4. ✅ `duroxide_orchestration_completions_total{orchestration_name, version, status, final_turn_count}`
5. ✅ `duroxide_orchestration_failures_total{orchestration_name, error_type}`
6. ✅ `duroxide_orchestration_history_size{orchestration_name}`
7. ✅ `duroxide_orchestration_turns{orchestration_name}`
8. ✅ `duroxide_orchestration_continue_as_new_total{orchestration_name, execution_id}`

**Available But NOT Visualized:**
9. ⚠️ `duroxide_activity_app_errors_total{activity_name}` - Exists, but redundant with executions{outcome="app_error"}
10. ❓ Infrastructure error metrics (need to verify if exported)
11. ❓ Configuration error metrics (need to verify if exported)
12. ❓ Provider metrics (need to verify if exported)
13. ❓ Client metrics (need to verify if exported)

---

## Verification Needed

When observability stack is running, check:

```bash
# Get all duroxide metrics
curl -s "http://localhost:8889/metrics" | grep "^duroxide_" | cut -d'{' -f1 | sort -u

# Get all available with descriptions
curl -s "http://localhost:8889/metrics" | grep "^# HELP duroxide"
```

**Expected additional metrics** (from observability guide):
- Infrastructure error counters (3 types)
- Configuration error counters (2 types)
- Provider latency histograms (3 types)
- Client operation counters (4 types)

---

## Dashboard Gap Analysis

### What We Have vs What We Need

#### For Production Operations:

**✅ Can Monitor:**
- Orchestration throughput (starts/completions)
- Success/failure rates
- Activity performance (duration percentiles)
- Activity success rates
- Orchestration complexity (turns, history size)
- Continue-as-new operations

**❌ Cannot Monitor:**
- Database performance (fetch/ack latency)
- Infrastructure vs application errors
- Configuration/deployment issues
- Database retry rates
- Client API usage

**⚠️ Partially Monitored:**
- Errors (have totals, but can't distinguish infrastructure from app)

#### For Debugging:

**✅ Can Debug:**
- Which activities are slow (duration histogram)
- Which orchestrations are failing (failure counter)
- Success rates by activity/orchestration
- Turn counts (complexity indicator)

**❌ Cannot Debug:**
- Is the problem in my code or the database?
- Are retries happening due to deadlocks?
- Which database operations are slow?
- Are there nondeterminism errors?

---

## Proposed Dashboard Updates

### Update: "Toygres Production Metrics"

**Add these panels:**

#### Row: "Infrastructure Health"
1. **Infrastructure Error Rate** (Time Series)
   ```promql
   rate(duroxide_*_infrastructure_errors_total[1m]) by (__name__, operation)
   ```

2. **Configuration Errors** (Stat - should be 0!)
   ```promql
   sum(duroxide_*_configuration_errors_total)
   ```

#### Row: "Database Performance"
3. **Database Operation Latency** (Time Series)
   ```promql
   histogram_quantile(0.95,
     rate(duroxide_provider_fetch_orchestration_item_duration_ms_bucket[5m])
   )
   ```

4. **Database Retry Rate** (Gauge)
   ```promql
   rate(duroxide_provider_ack_orchestration_retries_total[5m])
   ```

---

### New Dashboard: "Toygres Database Health"

**Purpose:** Deep dive into provider/database performance

**Panels:**
1. Fetch latency (p50, p95, p99)
2. Ack latency (p50, p95, p99)
3. Worker ack latency
4. Retry rate over time
5. Infrastructure error breakdown
6. Operation rate (fetches/sec, acks/sec)

---

## Action Items

### Immediate (Next Session):

1. **Verify which metrics are actually being exported:**
   ```bash
   ./scripts/start-observability.sh
   sleep 20
   curl "http://localhost:8889/metrics" | grep "^duroxide_" | cut -d'{' -f1 | sort -u > /tmp/actual-metrics.txt
   cat /tmp/actual-metrics.txt
   ```

2. **Add missing panels to Production dashboard:**
   - Infrastructure error rate
   - Configuration error alert
   - Database latency (if available)

3. **Create "Database Health" dashboard** (if provider metrics available)

4. **Update METRICS-COMPARISON.md** with actual availability

### Future (When Duroxide Stabilizes):

5. **Add client metrics** (API usage tracking)
6. **Add worker metrics** (queue depth, processing rate)
7. **Add active orchestrations gauge** (when duroxide implements it)

---

## Example: Complete Dashboard with All Metrics

### Ideal "Toygres Complete Observability" Dashboard

**Row 1: Executive Summary**
- Total active orchestrations (gauge) - ⚠️ NEEDS DUROXIDE FIX
- Orchestration success rate (gauge) - ✅ Have
- Activity error rate (gauge) - ✅ Have
- Infrastructure error rate (gauge) - ❌ Missing

**Row 2: Orchestration Performance**
- Orchestration duration p95 (time series) - ✅ Have
- Orchestration starts by type (time series) - ✅ Have
- Orchestration failures by error (time series) - ✅ Have
- History size trend (time series) - ✅ Have

**Row 3: Activity Performance**
- Activity duration percentiles (time series) - ✅ Have
- Activity success rate by activity (time series) - ✅ Have
- Activity retries (time series) - ⚠️ Can calculate, not in dashboard
- Activity infrastructure errors (time series) - ❌ Missing

**Row 4: Database Health**
- Fetch latency p95 (time series) - ❌ Missing
- Ack latency p95 (time series) - ❌ Missing
- Database retry rate (gauge) - ❌ Missing
- Provider errors (time series) - ❌ Missing

**Row 5: Configuration Issues**
- Configuration errors (table) - ❌ Missing
- Nondeterminism errors (stat - should be 0!) - ❌ Missing
- Unregistered orchestration/activity calls (stat) - ❌ Missing

**Row 6: Client Operations**
- Client orchestration starts (time series) - ❌ Missing
- Events raised (time series) - ❌ Missing
- Client cancellations (counter) - ❌ Missing

**Coverage:** 8/23 metrics visualized = **35%**

---

## Recommendations

### For Toygres Administrator (You):

1. **Use existing dashboards** for what you can monitor (35% is still valuable!)
2. **When observability stack is running, verify** which additional metrics are actually exported
3. **Add high-priority panels** for infrastructure errors and database performance
4. **Document gaps** for duroxide framework improvements

### For Duroxide Framework (Also You):

1. **Ensure all documented metrics are actually exported** (verify provider metrics)
2. **Fix the active_orchestrations gauge** (in progress, has bug in commit 3671ac16)
3. **Add provider metrics** if not yet implemented
4. **Document which metrics require feature flags** or configuration

---

## Summary

**Current Status:**
- ✅ **8 out of 23 metrics** (35%) actively used in dashboards
- ✅ **Core monitoring works** - orchestration/activity performance visible
- ⚠️ **Infrastructure visibility missing** - can't distinguish DB issues from app bugs
- ❌ **Database performance blind** - no provider metrics in dashboard
- ⚠️ **Active count broken** - need gauge metric from duroxide

**Next Steps:**
1. Start observability stack
2. Verify all available metrics
3. Add infrastructure error panels
4. Add database performance panels (if available)
5. Update this comparison with actual state

---

**See `observability/OBSERVABILITY-STATUS.md` for current operational status.**

