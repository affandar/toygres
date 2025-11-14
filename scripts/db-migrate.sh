#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ENV_FILE="${REPO_ROOT}/.env"
MIGRATIONS_DIR="${REPO_ROOT}/migrations/cms"
MIGRATION_TABLE="toygres_cms._toygres_migrations"

if [[ -f "${ENV_FILE}" ]]; then
    # shellcheck disable=SC2046
    export $(grep -v '^#' "${ENV_FILE}" | xargs)
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "Error: DATABASE_URL not set in .env file"
    exit 1
fi

psql "${DATABASE_URL}" <<SQL
CREATE SCHEMA IF NOT EXISTS toygres_cms;

CREATE TABLE IF NOT EXISTS ${MIGRATION_TABLE} (
    version BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SQL

if [[ ! -d "${MIGRATIONS_DIR}" ]]; then
    echo "No migrations directory found at ${MIGRATIONS_DIR}"
    exit 0
fi

shopt -s nullglob
applied_any=false

for file in "${MIGRATIONS_DIR}"/[0-9][0-9][0-9][0-9]_*.sql; do
    base="$(basename "${file}")"
    version_prefix="${base%%_*}"
    version=$((10#${version_prefix}))
    name="${base#*_}"
    escaped_name="${name//\'/\'\'}"

    # Skip the initial schema (handled by db-init)
    if [[ "${version}" -eq 1 ]]; then
        continue
    fi

    already_applied=$(psql "${DATABASE_URL}" -tAc "SELECT 1 FROM ${MIGRATION_TABLE} WHERE version = ${version}")
    if [[ -n "${already_applied// /}" ]]; then
        echo "↷ Skipping ${base} (already applied)"
        continue
    fi

    echo "→ Applying migration ${base}..."
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -f "${file}"
    psql "${DATABASE_URL}" <<SQL
INSERT INTO ${MIGRATION_TABLE} (version, name)
VALUES (${version}, '${escaped_name}')
ON CONFLICT (version) DO NOTHING;
SQL

    applied_any=true
    echo "✓ Migration ${base} applied"
done

if [[ "${applied_any}" == false ]]; then
    echo "No pending migrations to apply."
fi

