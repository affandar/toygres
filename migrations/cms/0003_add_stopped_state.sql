-- 0003_add_stopped_state.sql
-- Description: Add 'stopped' state for instance lifecycle management

-- Add 'stopped' value to instance_state enum
-- PostgreSQL requires ALTER TYPE to add new enum values
DO $$
BEGIN
    -- Check if 'stopped' value already exists
    IF NOT EXISTS (
        SELECT 1
        FROM pg_enum e
        JOIN pg_type t ON e.enumtypid = t.oid
        WHERE t.typname = 'instance_state' AND e.enumlabel = 'stopped'
    ) THEN
        ALTER TYPE public.instance_state ADD VALUE 'stopped' AFTER 'running';
    END IF;
END;
$$;

-- Add column to track when instance was stopped/started
ALTER TABLE toygres_cms.instances 
    ADD COLUMN IF NOT EXISTS stopped_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ;

-- Add index for querying stopped instances
CREATE INDEX IF NOT EXISTS idx_instances_stopped 
    ON toygres_cms.instances(state) 
    WHERE state = 'stopped';
