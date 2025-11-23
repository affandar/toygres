#!/bin/bash

# Toygres Control Plane Shutdown Script
# Stops: Log Forwarder + Toygres Server + Web UI + Observability Stack

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PIDS_DIR="$PROJECT_ROOT/.pids"
LOG_FORWARDER_PID_FILE="$PIDS_DIR/log-forwarder.pid"
UI_PID_FILE="$PIDS_DIR/ui.pid"

echo ""
echo -e "${YELLOW}🛑 Stopping Toygres Control Plane${NC}"
echo "========================================"
echo ""

# Stop log forwarder
if [[ -f "$LOG_FORWARDER_PID_FILE" ]]; then
    LOG_FORWARDER_PID=$(cat "$LOG_FORWARDER_PID_FILE")
    if ps -p "$LOG_FORWARDER_PID" > /dev/null 2>&1; then
        echo "📝 Stopping log forwarder (PID: $LOG_FORWARDER_PID)..."
        kill "$LOG_FORWARDER_PID" 2>/dev/null || true
        # Wait for process to die
        for i in {1..5}; do
            if ! ps -p "$LOG_FORWARDER_PID" > /dev/null 2>&1; then
                break
            fi
            sleep 1
        done
        # Force kill if still running
        if ps -p "$LOG_FORWARDER_PID" > /dev/null 2>&1; then
            kill -9 "$LOG_FORWARDER_PID" 2>/dev/null || true
        fi
    fi
    rm -f "$LOG_FORWARDER_PID_FILE"
    echo -e "${GREEN}✓ Log forwarder stopped${NC}"
else
    echo "📝 Log forwarder not running"
fi

# Stop Web UI
if [[ -f "$UI_PID_FILE" ]]; then
    UI_PID=$(cat "$UI_PID_FILE")
    if ps -p "$UI_PID" > /dev/null 2>&1; then
        echo "🎨 Stopping Web UI (PID: $UI_PID)..."
        kill "$UI_PID" 2>/dev/null || true
        for i in {1..5}; do
            if ! ps -p "$UI_PID" > /dev/null 2>&1; then
                break
            fi
            sleep 1
        done
        if ps -p "$UI_PID" > /dev/null 2>&1; then
            kill -9 "$UI_PID" 2>/dev/null || true
        fi
    fi
    rm -f "$UI_PID_FILE"
    echo -e "${GREEN}✓ Web UI stopped${NC}"
else
    echo "🎨 Web UI not running"
fi

# Stop Toygres server
echo "🚀 Stopping Toygres server..."
./toygres server stop 2>/dev/null || echo "   (already stopped)"
echo -e "${GREEN}✓ Toygres server stopped${NC}"

# Stop observability stack
echo "📊 Stopping observability stack..."
docker compose -f docker-compose.observability.yml down

# Clean option to remove volumes
if [[ "$1" == "--clean" ]]; then
    echo "🧹 Removing volumes..."
    docker compose -f docker-compose.observability.yml down -v
    echo -e "${GREEN}✓ Volumes removed${NC}"
fi

echo -e "${GREEN}✓ Observability stack stopped${NC}"

# Clean up PID directory if empty
if [[ -d "$PIDS_DIR" ]] && [[ -z "$(ls -A "$PIDS_DIR")" ]]; then
    rmdir "$PIDS_DIR"
fi

echo ""
echo -e "${GREEN}✅ All services stopped${NC}"
echo ""

# Show what's still running (if anything)
if docker compose -f docker-compose.observability.yml ps 2>/dev/null | grep -q "Up"; then
    echo -e "${YELLOW}⚠  Some containers are still running:${NC}"
    docker compose -f docker-compose.observability.yml ps
    echo ""
fi


