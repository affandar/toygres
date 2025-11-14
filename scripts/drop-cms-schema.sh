#!/bin/bash
set -euo pipefail

echo "Drop CMS Schema"
echo "==============="
echo ""

if [[ -f .env ]]; then
    export $(grep -v '^#' .env | xargs)
else
    echo "Error: .env file not found. Copy .env.example and configure it first."
    exit 1
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "Error: DATABASE_URL not set in .env file"
    exit 1
fi

echo "This will drop schema 'toygres_cms' including all Toygres instance metadata."
read -rp "Are you sure? (y/N): " confirm

if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "Aborted."
    exit 0
fi

psql "$DATABASE_URL" <<'SQL'
DROP SCHEMA IF EXISTS toygres_cms CASCADE;
SQL

echo "Schema 'toygres_cms' dropped."

