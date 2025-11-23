#!/bin/bash
set -e

# Start Toygres backend and frontend for development

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🚀 Starting Toygres Development Environment"
echo "==========================================="
echo ""

# Check if .env exists
if [[ ! -f "$PROJECT_ROOT/.env" ]]; then
    echo "❌ Error: .env file not found"
    echo "   Copy .env.example to .env and configure it first"
    exit 1
fi

# Load .env
source "$PROJECT_ROOT/.env"

# Load observability config if available
if [[ -f "$PROJECT_ROOT/observability/env.local.example" ]]; then
    echo "📊 Loading observability configuration..."
    source "$PROJECT_ROOT/observability/env.local.example"
fi

# Check DATABASE_URL
if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "❌ Error: DATABASE_URL not set in .env"
    exit 1
fi

# Check if backend binary exists
if [[ ! -f "$PROJECT_ROOT/target/debug/toygres-server" ]]; then
    echo "📦 Building backend..."
    cd "$PROJECT_ROOT"
    cargo build --workspace
fi

# Stop any existing server
echo "🔄 Checking for existing server..."
cd "$PROJECT_ROOT"
./toygres server stop 2>/dev/null || true

# Start backend
echo "🚀 Starting Toygres server..."
./toygres server start

# Start log forwarder if Loki is available
if curl -s http://localhost:3100/ready > /dev/null 2>&1; then
    echo "📊 Starting log forwarder to Loki..."
    nohup ./scripts/push-logs-to-loki.sh > /dev/null 2>&1 &
    LOG_FORWARDER_PID=$!
    echo "   Log forwarder PID: $LOG_FORWARDER_PID"
fi

# Wait for backend to be ready
echo "⏳ Waiting for backend to be ready..."
for i in {1..30}; do
    if curl -s http://localhost:8080/health > /dev/null 2>&1; then
        echo "✅ Backend ready"
        break
    fi
    if [[ $i -eq 30 ]]; then
        echo "❌ Backend failed to start. Check logs:"
        echo "   ./toygres server logs"
        exit 1
    fi
    sleep 1
done

# Start frontend
echo "🎨 Starting Web UI..."
cd "$PROJECT_ROOT/toygres-ui"

if [[ ! -d "node_modules" ]]; then
    echo "📦 Installing frontend dependencies..."
    npm install
fi

echo ""
echo "✅ Toygres is ready!"
echo ""
echo "   Backend API: http://localhost:8080"
echo "   Web UI:      http://localhost:3000"
echo ""
echo "Press Ctrl+C to stop both services"
echo ""

npm start

