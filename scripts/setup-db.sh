#!/bin/bash
set -e

echo "Toygres Database Setup"
echo "======================"
echo ""

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
else
    echo "Error: .env file not found. Copy .env.example and configure it first."
    exit 1
fi

if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL not set in .env file"
    exit 1
fi

echo "Creating database schemas..."
echo ""

# Execute SQL to create the schemas
psql "$DATABASE_URL" <<EOF
-- Create schema for Duroxide orchestration state
CREATE SCHEMA IF NOT EXISTS toygres_duroxide;

-- Set search path to include both schemas
SET search_path TO public, toygres_duroxide;

-- Create custom types
CREATE TYPE instance_state AS ENUM ('creating', 'running', 'deleting', 'deleted', 'failed');
CREATE TYPE health_status AS ENUM ('healthy', 'unhealthy', 'unknown');

-- Create instances table
CREATE TABLE IF NOT EXISTS instances (
    id UUID PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    state instance_state NOT NULL,
    health_status health_status NOT NULL DEFAULT 'unknown',
    connection_string TEXT,
    health_check_orchestration_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_instances_state ON instances(state);
CREATE INDEX IF NOT EXISTS idx_instances_health_status ON instances(health_status);
CREATE INDEX IF NOT EXISTS idx_instances_created_at ON instances(created_at);

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS \$\$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
\$\$ language 'plpgsql';

-- Create trigger
DROP TRIGGER IF EXISTS update_instances_updated_at ON instances;
CREATE TRIGGER update_instances_updated_at
    BEFORE UPDATE ON instances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

EOF

echo ""
echo "Database schemas created successfully!"
echo ""
echo "Schemas:"
echo "  - public (application metadata)"
echo "  - toygres_duroxide (Duroxide orchestration state)"
echo ""
echo "Tables in public schema:"
echo "  - instances"
echo ""
echo "Note: Duroxide will create its own tables in toygres_duroxide schema automatically."
echo ""
echo "You can now run the Toygres control plane."

