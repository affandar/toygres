#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ENV_FILE="${REPO_ROOT}/.env"
MIGRATIONS_DIR="${REPO_ROOT}/migrations/cms"
INITIAL_MIGRATION="${MIGRATIONS_DIR}/0001_initial_schema.sql"
MIGRATION_TABLE="toygres_cms._toygres_migrations"

if [[ -f "${ENV_FILE}" ]]; then
    # shellcheck disable=SC2046
    export $(grep -v '^#' "${ENV_FILE}" | xargs)
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "Error: DATABASE_URL not set in .env file"
    exit 1
fi

if [[ ! -f "${INITIAL_MIGRATION}" ]]; then
    echo "Error: Initial migration file not found at ${INITIAL_MIGRATION}"
    exit 1
fi

echo "Toygres Database Init"
echo "====================="
echo ""

psql "${DATABASE_URL}" <<SQL
CREATE SCHEMA IF NOT EXISTS toygres_cms;

CREATE TABLE IF NOT EXISTS ${MIGRATION_TABLE} (
    version BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SQL

version_applied=$(psql "${DATABASE_URL}" -tAc "SELECT 1 FROM ${MIGRATION_TABLE} WHERE version = 1")

if [[ -n "${version_applied// /}" ]]; then
    echo "✓ Initial CMS schema already applied (version 1)"
else
    echo "Applying initial CMS schema (0001_initial_schema.sql)..."
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -f "${INITIAL_MIGRATION}"
    psql "${DATABASE_URL}" <<SQL
INSERT INTO ${MIGRATION_TABLE} (version, name)
VALUES (1, '0001_initial_schema')
ON CONFLICT (version) DO NOTHING;
SQL
    echo "✓ Initial CMS schema applied"
fi

echo ""
echo "Checking for additional migrations..."
"${SCRIPT_DIR}/db-migrate.sh" --skip-initial-check

