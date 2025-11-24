#!/bin/bash

# Restart toygres server with DEBUG logging

echo "Stopping any running toygres-server..."
pkill -9 toygres-server 2>/dev/null

echo "Starting toygres-server with DEBUG logging..."

# Set debug log levels
export RUST_LOG=debug,toygres_server=debug,toygres_orchestrations=debug,duroxide=debug
export DUROXIDE_LOG_LEVEL=debug
export DUROXIDE_OBSERVABILITY_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export DUROXIDE_LOG_FORMAT=json

cd /Users/affandar/workshop/toygres

# Start server
cargo run --bin toygres-server -- standalone --port 8080

