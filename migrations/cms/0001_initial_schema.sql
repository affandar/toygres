-- 0001_initial_schema.sql
-- Description: Initial CMS schema for Toygres control plane

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE SCHEMA IF NOT EXISTS toygres_duroxide;
CREATE SCHEMA IF NOT EXISTS toygres_cms;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'instance_state' AND n.nspname = 'public'
    ) THEN
        CREATE TYPE public.instance_state AS ENUM ('creating', 'running', 'deleting', 'deleted', 'failed');
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'health_status' AND n.nspname = 'public'
    ) THEN
        CREATE TYPE public.health_status AS ENUM ('healthy', 'unhealthy', 'unknown');
    END IF;
END;
$$;

SET search_path TO toygres_cms, public;

CREATE TABLE IF NOT EXISTS instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_name VARCHAR(255) NOT NULL,
    k8s_name VARCHAR(255) UNIQUE NOT NULL,
    namespace VARCHAR(255) NOT NULL DEFAULT 'toygres',
    postgres_version VARCHAR(50) NOT NULL,
    storage_size_gb INTEGER NOT NULL,
    use_load_balancer BOOLEAN NOT NULL DEFAULT true,
    dns_name VARCHAR(255),
    ip_connection_string TEXT,
    dns_connection_string TEXT,
    external_ip VARCHAR(45),
    state instance_state NOT NULL DEFAULT 'creating',
    health_status health_status NOT NULL DEFAULT 'unknown',
    create_orchestration_id TEXT NOT NULL,
    delete_orchestration_id TEXT,
    instance_actor_orchestration_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    tags JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_instances_user_name ON instances(user_name);
CREATE INDEX IF NOT EXISTS idx_instances_k8s_name ON instances(k8s_name);
CREATE INDEX IF NOT EXISTS idx_instances_state ON instances(state);
CREATE INDEX IF NOT EXISTS idx_instances_health_status ON instances(health_status) WHERE state = 'running';
CREATE INDEX IF NOT EXISTS idx_instances_namespace ON instances(namespace);
CREATE INDEX IF NOT EXISTS idx_instances_tags ON instances USING gin(tags);

CREATE UNIQUE INDEX IF NOT EXISTS idx_instances_dns_name_unique
    ON instances(dns_name)
    WHERE dns_name IS NOT NULL
      AND dns_name NOT LIKE '__deleted_%'
      AND state IN ('creating', 'running');

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS update_instances_updated_at ON instances;
CREATE TRIGGER update_instances_updated_at
    BEFORE UPDATE ON instances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE IF NOT EXISTS instance_events (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    old_state VARCHAR(50),
    new_state VARCHAR(50),
    message TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_instance_events_instance_id ON instance_events(instance_id);
CREATE INDEX IF NOT EXISTS idx_instance_events_type ON instance_events(event_type);
CREATE INDEX IF NOT EXISTS idx_instance_events_created_at ON instance_events(created_at DESC);

CREATE TABLE IF NOT EXISTS instance_health_checks (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL,
    postgres_version TEXT,
    response_time_ms INTEGER,
    error_message TEXT,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_health_check_time UNIQUE (instance_id, checked_at)
);

CREATE INDEX IF NOT EXISTS idx_health_checks_instance_id ON instance_health_checks(instance_id);
CREATE INDEX IF NOT EXISTS idx_health_checks_checked_at ON instance_health_checks(checked_at DESC);

CREATE TABLE IF NOT EXISTS drift_detections (
    id BIGSERIAL PRIMARY KEY,
    detection_type VARCHAR(100) NOT NULL,
    k8s_name VARCHAR(255),
    cms_state VARCHAR(50),
    k8s_exists BOOLEAN,
    message TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolved_by VARCHAR(100)
);

CREATE INDEX IF NOT EXISTS idx_drift_detections_type ON drift_detections(detection_type);
CREATE INDEX IF NOT EXISTS idx_drift_detections_detected_at ON drift_detections(detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_drift_detections_unresolved ON drift_detections(resolved_at) WHERE resolved_at IS NULL;

