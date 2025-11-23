#!/bin/bash

# Push toygres server logs to Loki
# This is a workaround for Promtail volume mount issues on macOS

LOKI_URL="${LOKI_URL:-http://localhost:3100}"

# Function to push log line to Loki
push_to_loki() {
    local log_line="$1"
    local timestamp_ns=$(date +%s%N)
    
    # Create JSON payload
    cat <<EOF | curl -s -X POST "$LOKI_URL/loki/api/v1/push" \
        -H "Content-Type: application/json" \
        --data-binary @- > /dev/null
{
  "streams": [
    {
      "stream": {
        "job": "toygres-server",
        "level": "info"
      },
      "values": [
        ["$timestamp_ns", "$log_line"]
      ]
    }
  ]
}
EOF
}

echo "Tailing toygres logs and pushing to Loki at $LOKI_URL"
echo "Press Ctrl+C to stop"
echo ""

# Follow toygres logs and push each line to Loki
cd /Users/affandar/workshop/toygres
./toygres server logs -f 2>&1 | grep -v "Following logs from:" | grep -v "Press Ctrl+C to stop" | while IFS= read -r line; do
    # Skip empty lines
    [[ -z "$line" ]] && continue
    # Extract log level if possible
    if [[ "$line" =~ ERROR ]]; then
        level="error"
    elif [[ "$line" =~ WARN ]]; then
        level="warn"
    else
        level="info"
    fi
    
    # Escape quotes in log line
    escaped_line=$(echo "$line" | sed 's/"/\\"/g')
    
    # Push to Loki
    timestamp_ns=$(date +%s%N)
    curl -s -X POST "$LOKI_URL/loki/api/v1/push" \
        -H "Content-Type: application/json" \
        --data-binary @- > /dev/null <<EOF
{
  "streams": [{
    "stream": {
      "job": "toygres",
      "level": "$level",
      "source": "server"
    },
    "values": [["$timestamp_ns", "$escaped_line"]]
  }]
}
EOF
    
    # Also print to console
    echo "$line"
done

