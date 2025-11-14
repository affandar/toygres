#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "scripts/setup-db.sh is deprecated. Running db-init.sh instead..."
"${SCRIPT_DIR}/db-init.sh" "$@"

