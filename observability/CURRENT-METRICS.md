# Duroxide Metrics: Currently Available in Toygres

**Last Updated:** 2025-11-22  
**Duroxide Commit:** 0e10cd58

## Metrics Currently Exported

### Total Count: 9 Metrics (17 time series with histogram buckets)

---

### 1. Orchestration Metrics (6 metrics)

#### `duroxide_orchestration_starts_total` ✅
**Type:** Counter  
**Labels:** `orchestration_name`, `version`, `initiated_by`  
**Description:** Total orchestrations started  
**Example:**
```
duroxide_orchestration_starts_total{
  orchestration_name="toygres-orchestrations::orchestration::instance-actor",
  version="1.0.0",
  initiated_by="continueAsNew"
} = 1150
```

#### `duroxide_orchestration_completions_total` ✅
**Type:** Counter  
**Labels:** `orchestration_name`, `version`, `status`, `final_turn_count`  
**Description:** Orchestrations completed  
**Example:**
```
duroxide_orchestration_completions_total{
  orchestration_name="...",
  status="continuedAsNew",
  final_turn_count="1-5"
} = 1098
```

#### `duroxide_orchestration_failures_total` ✅
**Type:** Counter  
**Labels:** `orchestration_name`, `error_type`  
**Description:** Orchestration failures

#### `duroxide_orchestration_duration_seconds` ✅
**Type:** Histogram  
**Labels:** `orchestration_name`, `status`  
**Buckets:** 0.1, 0.5, 1, 5, 10, 30, 60, 300, 600, 1800 seconds  
**Description:** End-to-end orchestration execution time

#### `duroxide_orchestration_history_size` ✅
**Type:** Histogram  
**Labels:** `orchestration_name`  
**Buckets:** 10, 50, 100, 500, 1000, 5000, 10000 events  
**Description:** History event count at completion

#### `duroxide_orchestration_turns` ✅
**Type:** Histogram  
**Labels:** `orchestration_name`  
**Buckets:** 1, 2, 5, 10, 20, 50, 100, 200, 500 turns  
**Description:** Number of turns to completion

---

### 2. Activity Metrics (2 metrics)

#### `duroxide_activity_executions_total` ✅
**Type:** Counter  
**Labels:** `activity_name`, `outcome`, `retry_attempt`  
**Description:** Activity execution outcomes  
**Example:**
```
duroxide_activity_executions_total{
  activity_name="toygres-orchestrations::activity::cms-record-health-check",
  outcome="success",
  retry_attempt="0"
} = 847
```

#### `duroxide_activity_duration_seconds` ✅
**Type:** Histogram  
**Labels:** `activity_name`, `outcome`  
**Buckets:** 0.01, 0.05, 0.1, 0.5, 1, 2, 5, 10, 30, 60, 120, 300 seconds  
**Description:** Activity execution time

---

### 3. Continue-as-New Tracking (1 metric)

#### `duroxide_orchestration_continue_as_new_total` ✅
**Type:** Counter  
**Labels:** `orchestration_name`, `execution_id`  
**Description:** Continue-as-new operations

---

## Metrics NOT Available (Yet)

### Provider / Database Metrics ❌
- `duroxide_provider_operation_duration_seconds`
- `duroxide_provider_ack_orchestration_retries_total`
- `duroxide_provider_infrastructure_errors_total`

### Infrastructure Error Metrics ❌
- `duroxide_orchestration_infrastructure_errors_total`
- `duroxide_activity_infrastructure_errors_total`

### Configuration Error Metrics ❌
- `duroxide_orchestration_configuration_errors_total`
- `duroxide_activity_configuration_errors_total`

### Client Metrics ❌
- `duroxide_client_orchestration_starts_total`
- `duroxide_client_external_events_raised_total`
- `duroxide_client_cancellations_total`
- `duroxide_client_wait_duration_seconds`

### Sub-Orchestration Metrics ❌
- `duroxide_suborchestration_calls_total`
- `duroxide_suborchestration_duration_seconds`

### Active Orchestrations Gauge ❌ (In Progress)
- `duroxide_active_orchestrations`
  - **Status:** Atomic counter exists in code
  - **Issue:** Not yet registered as OpenTelemetry gauge
  - **Needs:** Export logic to read atomic counter and expose as gauge

---

## What You Can Monitor NOW

### ✅ Orchestration Performance
```promql
# Success rate
sum(rate(duroxide_orchestration_completions_total{status="success"}[5m])) 
/ 
sum(rate(duroxide_orchestration_completions_total[5m]))

# Duration percentiles
histogram_quantile(0.95, 
  rate(duroxide_orchestration_duration_seconds_bucket[5m])
) by (orchestration_name)

# Failure rate
rate(duroxide_orchestration_failures_total[5m]) by (orchestration_name, error_type)
```

### ✅ Activity Performance
```promql
# Activity duration by name
histogram_quantile(0.99, 
  rate(duroxide_activity_duration_seconds_bucket[5m])
) by (activity_name)

# Success rate by activity
rate(duroxide_activity_executions_total{outcome="success"}[5m]) 
by (activity_name)

# Retry rate
rate(duroxide_activity_executions_total{retry_attempt!="0"}[5m])
```

### ✅ Continue-as-New Tracking
```promql
# Instance actor cycles
rate(duroxide_orchestration_continue_as_new_total[5m]) 
by (orchestration_name)

# Total continues
sum(duroxide_orchestration_continue_as_new_total)
```

### ✅ Orchestration Complexity
```promql
# Orchestrations with many turns (optimization targets)
histogram_quantile(0.99, 
  rate(duroxide_orchestration_turns_bucket[5m])
) by (orchestration_name)

# Large history sizes
histogram_quantile(0.99, 
  rate(duroxide_orchestration_history_size_bucket[5m])
) by (orchestration_name)
```

---

## What You CANNOT Monitor (Yet)

### ❌ Accurate Active Count
- Can't distinguish active from completed
- Can't get real-time orchestration count
- **Workaround:** Query CMS database

### ❌ Database Performance
- Can't see fetch/ack latency
- Can't identify slow queries
- Can't track database retries

### ❌ Error Classification
- Can't distinguish infrastructure from app errors
- Can't identify database vs network issues

### ❌ Configuration Issues
- Can't detect nondeterminism errors
- Can't track unregistered orchestrations

### ❌ Client API Usage
- Can't track API call rates
- Can't monitor event raising

---

## Dashboards Status

### ✅ Working Dashboards (Use These!)

1. **"Toygres Production Metrics"** - Best for monitoring
   - Uses all 9 available metrics
   - Performance & success tracking
   - 100% functional

2. **"Toygres Simple"** - Quick overview
   - Basic health monitoring
   - 100% functional

3. **"Toygres Logs"** - Debugging
   - Log aggregation
   - 100% functional

### ⚠️ Partially Working

4. **"Toygres Active Orchestrations"** - Needs active gauge
   - Shows data but calculation is incorrect
   - Use CMS database as workaround

### ⏳ Future (When Metrics Added)

5. **"Toygres Complete Observability"** - Comprehensive
   - Created and ready
   - Will work when duroxide adds:
     - `duroxide_active_orchestrations` gauge
     - Provider metrics
     - Infrastructure error metrics
     - Client metrics

---

## Recommended Approach

### For Now (Production Ready):

Use **"Toygres Production Metrics"** dashboard:
- ✅ Orchestration throughput and success rate
- ✅ Activity duration percentiles
- ✅ Error tracking
- ✅ Continue-as-new monitoring
- ✅ History size and complexity tracking

### For Active Count:

Query CMS database:
```sql
SELECT state, COUNT(*) as count 
FROM cms.instances 
WHERE state IN ('pending', 'running') 
GROUP BY state;
```

Or via API:
```bash
curl http://localhost:8080/api/instances | jq '[.[] | select(.state == "running")] | length'
```

### When Duroxide Adds More Metrics:

- Switch to "Toygres Complete Observability" dashboard
- Get database performance visibility
- Get accurate active count
- Get infrastructure error classification

---

## Summary

**Current Reality:**
- 9 core metrics fully functional
- Rich labels (activity_name, orchestration_name, outcome, etc.)
- Performance monitoring excellent
- Error tracking good (but no infra vs app distinction)
- Active count requires workaround

**Coming Soon (Duroxide Roadmap):**
- Active orchestrations gauge
- Provider/database metrics
- Infrastructure error classification
- Client operation tracking

**What This Means:**
- ✅ **Current dashboards are production-ready for performance monitoring**
- ⚠️ **Some operational gaps** (active count, database performance)
- 📋 **Comprehensive dashboard ready** for when metrics are added

---

**Recommendation:** Use "Toygres Production Metrics" dashboard now. It works with the 9 available metrics and gives excellent visibility into orchestration and activity performance!

Access: http://localhost:3001/d/toygres-production

