#!/bin/bash

# Check status of observability stack
# Usage: ./scripts/observability-status.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "📊 Toygres Observability Stack Status"
echo "======================================"
echo ""

docker compose -f docker-compose.observability.yml ps

echo ""
echo "🔗 Service URLs:"
echo "  Grafana:    http://localhost:3001"
echo "  Prometheus: http://localhost:9090"
echo "  Loki:       http://localhost:3100"
echo ""

# Check if services are healthy
if curl -s http://localhost:3001/api/health > /dev/null 2>&1; then
    echo "✅ Grafana is healthy"
else
    echo "❌ Grafana is not responding"
fi

if curl -s http://localhost:9090/-/healthy > /dev/null 2>&1; then
    echo "✅ Prometheus is healthy"
else
    echo "❌ Prometheus is not responding"
fi

if curl -s http://localhost:3100/ready > /dev/null 2>&1; then
    echo "✅ Loki is healthy"
else
    echo "❌ Loki is not responding"
fi

if curl -s http://localhost:4317 > /dev/null 2>&1; then
    echo "✅ OTLP Collector is healthy"
else
    echo "❌ OTLP Collector is not responding"
fi


