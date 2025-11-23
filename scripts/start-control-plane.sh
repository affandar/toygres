#!/bin/bash
set -e

# Toygres Control Plane Startup Script
# Starts: Observability Stack + Toygres Server + Web UI + Log Forwarder

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# PID file locations
PIDS_DIR="$PROJECT_ROOT/.pids"
mkdir -p "$PIDS_DIR"

LOG_FORWARDER_PID_FILE="$PIDS_DIR/log-forwarder.pid"
UI_PID_FILE="$PIDS_DIR/ui.pid"

echo ""
echo -e "${BLUE}🚀 Starting Toygres Control Plane${NC}"
echo "========================================"
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Shutting down Toygres Control Plane...${NC}"
    
    # Stop log forwarder
    if [[ -f "$LOG_FORWARDER_PID_FILE" ]]; then
        LOG_FORWARDER_PID=$(cat "$LOG_FORWARDER_PID_FILE")
        if ps -p "$LOG_FORWARDER_PID" > /dev/null 2>&1; then
            echo "   Stopping log forwarder (PID: $LOG_FORWARDER_PID)..."
            kill "$LOG_FORWARDER_PID" 2>/dev/null || true
            # Kill child processes too (toygres server logs -f)
            pkill -P "$LOG_FORWARDER_PID" 2>/dev/null || true
            wait "$LOG_FORWARDER_PID" 2>/dev/null || true
        fi
        rm -f "$LOG_FORWARDER_PID_FILE"
    fi
    
    # Kill any orphaned log forwarders
    pkill -f "push-logs-to-loki.sh" 2>/dev/null || true
    pkill -f "toygres-server server logs" 2>/dev/null || true
    
    # Stop Web UI
    if [[ -f "$UI_PID_FILE" ]]; then
        UI_PID=$(cat "$UI_PID_FILE")
        if ps -p "$UI_PID" > /dev/null 2>&1; then
            echo "   Stopping Web UI (PID: $UI_PID)..."
            kill "$UI_PID" 2>/dev/null || true
            wait "$UI_PID" 2>/dev/null || true
        fi
        rm -f "$UI_PID_FILE"
    fi
    
    # Kill any orphaned npm/node processes from UI
    pkill -f "react-scripts start" 2>/dev/null || true
    
    # Stop Toygres server
    echo "   Stopping Toygres server..."
    "$PROJECT_ROOT/toygres" server stop 2>/dev/null || true
    
    # Stop observability stack
    echo "   Stopping observability stack..."
    cd "$PROJECT_ROOT"
    docker compose -f docker-compose.observability.yml down
    
    echo ""
    echo -e "${GREEN}✅ Shutdown complete${NC}"
    echo ""
    exit 0
}

# Register cleanup on Ctrl+C and exit
trap cleanup SIGINT SIGTERM EXIT

# Check prerequisites
echo -e "${BLUE}📋 Checking prerequisites...${NC}"

if [[ ! -f ".env" ]]; then
    echo -e "${RED}❌ Error: .env file not found${NC}"
    echo "   Copy .env.example to .env and configure it first"
    exit 1
fi

if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Error: Docker not found${NC}"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Error: Cargo not found${NC}"
    exit 1
fi

# Load environment
source "$PROJECT_ROOT/.env"
source "$PROJECT_ROOT/observability/env.local.example"

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo -e "${RED}❌ Error: DATABASE_URL not set in .env${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Prerequisites OK${NC}"
echo ""

# Step 1: Start Observability Stack
echo -e "${BLUE}📊 Starting observability stack...${NC}"
docker compose -f docker-compose.observability.yml up -d

# Wait for services to be ready
echo "   Waiting for services to be ready..."
sleep 5

# Check Grafana
if curl -s http://localhost:3001/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}   ✓ Grafana ready (http://localhost:3001)${NC}"
else
    echo -e "${YELLOW}   ⚠ Grafana not responding yet${NC}"
fi

# Check Prometheus
if curl -s http://localhost:9090/-/healthy > /dev/null 2>&1; then
    echo -e "${GREEN}   ✓ Prometheus ready (http://localhost:9090)${NC}"
else
    echo -e "${YELLOW}   ⚠ Prometheus not responding yet${NC}"
fi

# Check Loki
if curl -s http://localhost:3100/ready > /dev/null 2>&1; then
    echo -e "${GREEN}   ✓ Loki ready (http://localhost:3100)${NC}"
else
    echo -e "${YELLOW}   ⚠ Loki not responding yet${NC}"
fi

echo ""

# Step 2: Check if backend binary exists
if [[ ! -f "$PROJECT_ROOT/target/debug/toygres-server" ]]; then
    echo -e "${BLUE}📦 Building backend...${NC}"
    cargo build --workspace
    echo ""
fi

# Step 3: Stop any existing server
echo -e "${BLUE}🔄 Checking for existing server...${NC}"
./toygres server stop 2>/dev/null || true
sleep 1
echo ""

# Step 4: Start Toygres backend
echo -e "${BLUE}🚀 Starting Toygres server...${NC}"
./toygres server start

# Wait for backend to be ready
echo "   Waiting for backend to be ready..."
for i in {1..30}; do
    if curl -s http://localhost:8080/health > /dev/null 2>&1; then
        echo -e "${GREEN}   ✓ Backend ready (http://localhost:8080)${NC}"
        break
    fi
    if [[ $i -eq 30 ]]; then
        echo -e "${RED}   ❌ Backend failed to start. Check logs:${NC}"
        echo "      ./toygres server logs"
        exit 1
    fi
    sleep 1
done
echo ""

# Step 5: Start log forwarder
echo -e "${BLUE}📝 Starting log forwarder to Loki...${NC}"
nohup "$SCRIPT_DIR/push-logs-to-loki.sh" > /dev/null 2>&1 &
LOG_FORWARDER_PID=$!
echo "$LOG_FORWARDER_PID" > "$LOG_FORWARDER_PID_FILE"
echo -e "${GREEN}   ✓ Log forwarder started (PID: $LOG_FORWARDER_PID)${NC}"
echo ""

# Step 6: Start Web UI
echo -e "${BLUE}🎨 Starting Web UI...${NC}"
cd "$PROJECT_ROOT/toygres-ui"

if [[ ! -d "node_modules" ]]; then
    echo "   Installing frontend dependencies..."
    npm install
fi

# Start UI in background
npm start > /dev/null 2>&1 &
UI_PID=$!
echo "$UI_PID" > "$UI_PID_FILE"

# Wait for UI to be ready
echo "   Waiting for UI to be ready..."
for i in {1..30}; do
    if curl -s http://localhost:3000 > /dev/null 2>&1; then
        echo -e "${GREEN}   ✓ Web UI ready (http://localhost:3000)${NC}"
        break
    fi
    if [[ $i -eq 30 ]]; then
        echo -e "${YELLOW}   ⚠ UI took longer than expected to start${NC}"
        break
    fi
    sleep 1
done

echo ""
echo -e "${GREEN}✅ Toygres Control Plane is ready!${NC}"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${BLUE}📊 Services:${NC}"
echo "   Backend API:  http://localhost:8080"
echo "   Web UI:       http://localhost:3000"
echo "   Grafana:      http://localhost:3001 (admin/admin)"
echo "   Prometheus:   http://localhost:9090"
echo ""
echo -e "${BLUE}📋 Useful Commands:${NC}"
echo "   View logs:         ./toygres server logs"
echo "   Check status:      ./scripts/observability-status.sh"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop all services${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Keep script running
wait

