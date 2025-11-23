#!/bin/bash
set -e

# Start local observability stack with Docker Compose
# Usage: ./scripts/start-observability.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "🚀 Starting Toygres Observability Stack..."
echo ""

# Create logs directory for Promtail (required for volume mount)
mkdir -p "$PROJECT_ROOT/logs"
echo "✓ Created logs directory"

# Start docker compose stack
docker compose -f docker-compose.observability.yml up -d

echo ""
echo "✅ Observability stack started!"
echo ""
echo "📊 Services:"
echo "  - Grafana:         http://localhost:3001 (admin/admin)"
echo "  - Prometheus:      http://localhost:9090"
echo "  - Loki:            http://localhost:3100"
echo "  - OTLP Collector:  http://localhost:4317 (gRPC)"
echo ""
echo "🔍 View logs:"
echo "  docker compose -f docker-compose.observability.yml logs -f"
echo ""
echo "⚙️  To enable observability in toygres-server:"
echo "  source observability/env.local.example"
echo "  cargo run --bin toygres-server -- server"
echo ""

