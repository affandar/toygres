#!/bin/bash

# Force kill all toygres processes and containers
# Use this if Ctrl+C doesn't work or processes are stuck

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🔥 Force killing all toygres processes..."
echo ""

# Kill log forwarders
echo "Killing log forwarders..."
pkill -9 -f "push-logs-to-loki.sh" 2>/dev/null || true
pkill -9 -f "toygres-server server logs" 2>/dev/null || true

# Kill UI processes
echo "Killing UI processes..."
pkill -9 -f "react-scripts start" 2>/dev/null || true
pkill -9 -f "npm.*start" 2>/dev/null || true

# Stop toygres server
echo "Stopping toygres server..."
cd "$PROJECT_ROOT"
./toygres server stop 2>/dev/null || true

# Force stop containers
echo "Force stopping Docker containers..."
docker compose -f docker-compose.observability.yml kill
docker compose -f docker-compose.observability.yml down

# Clean PID files
rm -rf "$PROJECT_ROOT/.pids"

echo ""
echo "✅ All processes killed"
echo ""

# Show what's left
echo "Checking for remaining processes..."
ps aux | grep -E "toygres|push-logs|react-scripts" | grep -v grep || echo "(none)"
echo ""

# Show remaining containers
echo "Checking for remaining containers..."
docker ps | grep toygres || echo "(none)"
echo ""


