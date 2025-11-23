# Duroxide Metrics Available in Toygres

**Based on:** `/Users/affandar/workshop/duroxide/docs/metrics-specification.md`  
**Last Checked:** 2025-11-22

## Metrics Currently Available: 13/24 (54%)

### ✅ Fully Working (13 metrics)

| Metric | Type | Labels | Status |
|--------|------|--------|--------|
| `duroxide_orchestration_starts_total` | Counter | orchestration_name, version, initiated_by | ✅ WORKING |
| `duroxide_orchestration_completions_total` | Counter | orchestration_name, version, status, final_turn_count | ✅ WORKING |
| `duroxide_orchestration_failures_total` | Counter | orchestration_name, version, error_type, error_category | ✅ WORKING |
| `duroxide_orchestration_duration_seconds` | Histogram | orchestration_name, version, status | ✅ WORKING |
| `duroxide_orchestration_history_size` | Histogram | orchestration_name | ✅ WORKING |
| `duroxide_orchestration_turns` | Histogram | orchestration_name | ✅ WORKING |
| `duroxide_orchestration_continue_as_new_total` | Counter | orchestration_name, execution_id | ✅ WORKING |
| `duroxide_active_orchestrations` | Gauge | state | ✅ WORKING (value=4) |
| `duroxide_orchestrator_queue_depth` | Gauge | _(none)_ | ✅ WORKING (value=5) |
| `duroxide_worker_queue_depth` | Gauge | _(none)_ | ✅ WORKING (value=0) |
| `duroxide_activity_executions_total` | Counter | activity_name, outcome, retry_attempt | ✅ WORKING |
| `duroxide_activity_duration_seconds` | Histogram | activity_name, outcome | ✅ WORKING |
| `duroxide_provider_operation_duration_seconds` | Histogram | operation, status | ✅ WORKING |

### ⚠️ Defined But Need Testing (6 metrics)

These should exist but may only appear when specific conditions occur:

| Metric | Type | When It Appears | Check |
|--------|------|----------------|-------|
| `duroxide_orchestration_infrastructure_errors_total` | Counter | When infra errors occur | Need error to trigger |
| `duroxide_orchestration_configuration_errors_total` | Counter | When config errors occur | Need error to trigger |
| `duroxide_activity_infrastructure_errors_total` | Counter | When activity infra errors | Need error to trigger |
| `duroxide_activity_configuration_errors_total` | Counter | When activity config errors | Need error to trigger |
| `duroxide_provider_errors_total` | Counter | When provider errors occur | Need DB error to trigger |
| `duroxide_suborchestration_calls_total` | Counter | When sub-orch called | Spec says "not called" |
| `duroxide_suborchestration_duration_seconds` | Histogram | When sub-orch completes | Spec says "not called" |

### ❌ Not Implemented (4 metrics)

Per spec: "Client metrics are currently defined but not instrumented"

| Metric | Status |
|--------|--------|
| `duroxide_client_orchestration_starts_total` | ❌ Not instrumented |
| `duroxide_client_external_events_raised_total` | ❌ Not instrumented |
| `duroxide_client_cancellations_total` | ❌ Not instrumented |
| `duroxide_client_wait_duration_seconds` | ❌ Not instrumented |

---

## Key Findings from Spec

### New Metrics (vs our old understanding):

1. **Queue Depth Gauges** ✅ NEW!
   - `duroxide_orchestrator_queue_depth` - Orchestration backlog
   - `duroxide_worker_queue_depth` - Activity backlog
   - **Use for:** Capacity planning, scaling decisions

2. **Error Classification** ✅ Enhanced
   - Errors now split by `error_type`: app_error, infrastructure_error, config_error
   - Errors also have `error_category` for fine-grained classification

3. **Activity Errors Consolidated**
   - **NO separate** `duroxide_activity_app_errors_total`
   - All errors tracked in `duroxide_activity_executions_total{outcome="app_error"}`

### Label Changes:

**Orchestration Failures:**
```
OLD: {orchestration_name, error_type}
NEW: {orchestration_name, version, error_type, error_category}
```

**Activity Executions:**
```
Labels: {activity_name, outcome, retry_attempt}
Outcomes: "success" | "app_error" | "infra_error" | "config_error"
```

---

## Dashboard Updates Needed

Based on spec, update dashboards to:

### 1. Add Queue Depth Panels

```json
{
  "title": "Orchestrator Queue Depth",
  "targets": [{
    "expr": "duroxide_orchestrator_queue_depth"
  }],
  "type": "gauge"
}
```

### 2. Update Error Panels to Use error_category

```promql
# Instead of just error_type, now have error_category too
rate(duroxide_orchestration_failures_total[1m]) 
by (orchestration_name, error_type, error_category)
```

### 3. Activity Errors from executions_total

```promql
# App errors
rate(duroxide_activity_executions_total{outcome="app_error"}[1m])
by (activity_name)

# Infra errors  
rate(duroxide_activity_executions_total{outcome="infra_error"}[1m])
by (activity_name)

# Config errors
rate(duroxide_activity_executions_total{outcome="config_error"}[1m])
by (activity_name)
```

### 4. Remove Panels for Non-Existent Metrics

Remove or mark as "Future":
- Infrastructure error separate counters (use error_type in failures instead)
- Configuration error separate counters (use error_type in failures instead)
- Provider errors (spec says it exists, but need to verify)
- Client metrics (explicitly not instrumented)
- Sub-orchestration metrics (explicitly not called)

---

## Spec-Compliant Dashboard Structure

### Row 1: Executive Summary
- Active Orchestrations (gauge) ✅
- Success Rate ✅
- Orchestrator Queue Depth ✅ NEW
- Worker Queue Depth ✅ NEW

### Row 2: Orchestration Performance
- Orchestration Start Rate (by initiated_by) ✅
- Orchestration Completion Rate (by status) ✅
- Orchestration Duration Percentiles ✅
- Orchestration Failures (by error_type & error_category) ✅

### Row 3: Activity Performance
- Activity Duration Percentiles ✅
- Activity Execution Rate (by outcome: success/app_error/infra_error) ✅
- Activity Retries (retry_attempt!="0") ✅

### Row 4: Database Performance
- Provider Operation Latency (by operation) ✅
- Provider Error Rate (if available) ⚠️

### Row 5: Orchestration Complexity
- History Size Distribution ✅
- Turn Count Distribution ✅
- Continue-as-New Rate ✅

---

## Next: Create Spec-Compliant Dashboard

Should I create a new dashboard that:
1. Uses ALL 13 working metrics
2. Includes new queue depth gauges
3. Uses proper label names (error_category, etc.)
4. Removes panels for non-existent metrics
5. Organized by the spec's categories?

This will be the **definitive production dashboard** based on what duroxide actually provides! 📊

