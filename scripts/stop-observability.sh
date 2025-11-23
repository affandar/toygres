#!/bin/bash
set -e

# Stop local observability stack
# Usage: ./scripts/stop-observability.sh [--clean]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "🛑 Stopping Toygres Observability Stack..."

docker compose -f docker-compose.observability.yml down

if [[ "$1" == "--clean" ]]; then
    echo "🧹 Cleaning volumes..."
    docker compose -f docker-compose.observability.yml down -v
    echo "✅ Volumes cleaned"
fi

echo "✅ Observability stack stopped"


