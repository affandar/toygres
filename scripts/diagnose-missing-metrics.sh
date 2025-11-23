#!/bin/bash

# Diagnostic script to identify which duroxide metrics are missing

echo "🔍 Duroxide Metrics Diagnostic"
echo "==============================="
echo ""

# Check if observability stack is running
if ! curl -s http://localhost:8889/metrics > /dev/null 2>&1; then
    echo "❌ OTLP Collector not responding. Run: ./scripts/start-observability.sh"
    exit 1
fi

echo "✅ OTLP Collector is running"
echo ""

# Get currently exported metrics
CURRENT_METRICS=$(curl -s "http://localhost:8889/metrics" | grep "^# TYPE duroxide" | awk '{print $3}' | sort)
CURRENT_COUNT=$(echo "$CURRENT_METRICS" | wc -l | tr -d ' ')

echo "📊 Currently Exported: $CURRENT_COUNT metrics"
echo ""
echo "$CURRENT_METRICS"
echo ""
echo "---"
echo ""

# Expected metrics list
EXPECTED_METRICS="duroxide_active_orchestrations
duroxide_orchestration_starts_total
duroxide_orchestration_completions_total
duroxide_orchestration_failures_total
duroxide_orchestration_duration_seconds
duroxide_orchestration_history_size
duroxide_orchestration_turns
duroxide_orchestration_continue_as_new_total
duroxide_orchestration_infrastructure_errors_total
duroxide_activity_executions_total
duroxide_activity_duration_seconds
duroxide_activity_infrastructure_errors_total
duroxide_activity_configuration_errors_total
duroxide_provider_operation_duration_seconds
duroxide_provider_ack_orchestration_retries_total
duroxide_provider_infrastructure_errors_total
duroxide_client_orchestration_starts_total
duroxide_client_external_events_raised_total
duroxide_client_cancellations_total
duroxide_client_wait_duration_seconds
duroxide_suborchestration_calls_total
duroxide_suborchestration_duration_seconds"

echo "❌ Missing Metrics:"
echo ""

MISSING_COUNT=0
for metric in $EXPECTED_METRICS; do
    if ! echo "$CURRENT_METRICS" | grep -q "^$metric$"; then
        echo "  - $metric"
        MISSING_COUNT=$((MISSING_COUNT + 1))
    fi
done

echo ""
echo "---"
echo ""
echo "📈 Summary:"
echo "  Available: $CURRENT_COUNT / 22 expected"
echo "  Missing: $MISSING_COUNT"
echo ""

if [ "$MISSING_COUNT" -eq 0 ]; then
    echo "🎉 All metrics available!"
else
    echo "📋 Check implementation in duroxide:"
    echo "  1. Metric defined? grep metric_name /path/to/duroxide/src/runtime/observability.rs"
    echo "  2. Metric registered? Look for .build() call"
    echo "  3. Recording method? Look for pub fn record_..."
    echo "  4. Method called? grep record_method /path/to/duroxide/src/**/*.rs"
    echo ""
    echo "See: docs/DUROXIDE-MISSING-METRICS-GUIDE.md"
fi

echo ""
echo "🔗 Quick links:"
echo "  Grafana: http://localhost:3001"
echo "  Prometheus: http://localhost:9090"
echo "  Raw metrics: http://localhost:8889/metrics"

