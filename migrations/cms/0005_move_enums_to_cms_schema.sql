-- 0005_move_enums_to_cms_schema.sql
-- Description: Move all enums and functions from public schema to toygres_cms
-- This consolidates all Toygres objects into toygres_cms for cleaner isolation

-- ============================================================================
-- Move instance_state enum from public to toygres_cms
-- ============================================================================

-- Step 1: Create the enum in toygres_cms if it doesn't exist there
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'instance_state' AND n.nspname = 'toygres_cms'
    ) THEN
        -- Check if it exists in public (to migrate) or needs fresh creation
        IF EXISTS (
            SELECT 1
            FROM pg_type t
            JOIN pg_namespace n ON n.oid = t.typnamespace
            WHERE t.typname = 'instance_state' AND n.nspname = 'public'
        ) THEN
            -- Move from public to toygres_cms
            ALTER TYPE public.instance_state SET SCHEMA toygres_cms;
        ELSE
            -- Create fresh (shouldn't happen, but safety net)
            CREATE TYPE toygres_cms.instance_state AS ENUM (
                'creating', 'running', 'stopped', 'deleting', 'deleted', 'failed'
            );
        END IF;
    END IF;
END;
$$;

-- ============================================================================
-- Move health_status enum from public to toygres_cms
-- ============================================================================

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'health_status' AND n.nspname = 'toygres_cms'
    ) THEN
        IF EXISTS (
            SELECT 1
            FROM pg_type t
            JOIN pg_namespace n ON n.oid = t.typnamespace
            WHERE t.typname = 'health_status' AND n.nspname = 'public'
        ) THEN
            ALTER TYPE public.health_status SET SCHEMA toygres_cms;
        ELSE
            CREATE TYPE toygres_cms.health_status AS ENUM ('healthy', 'unhealthy', 'unknown');
        END IF;
    END IF;
END;
$$;

-- ============================================================================
-- Move update_updated_at_column function from public to toygres_cms
-- ============================================================================

-- Create the function in toygres_cms schema
CREATE OR REPLACE FUNCTION toygres_cms.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Recreate the trigger to use the new function location
DROP TRIGGER IF EXISTS update_instances_updated_at ON toygres_cms.instances;
CREATE TRIGGER update_instances_updated_at
    BEFORE UPDATE ON toygres_cms.instances
    FOR EACH ROW
    EXECUTE FUNCTION toygres_cms.update_updated_at_column();

-- Drop the old function from public schema (if it exists and no other dependencies)
DROP FUNCTION IF EXISTS public.update_updated_at_column();

-- ============================================================================
-- Update search_path comment for future migrations
-- ============================================================================
COMMENT ON SCHEMA toygres_cms IS 'Toygres CMS schema - contains all control plane metadata. Enums: instance_state, health_status, image_type. Tables: instances, instance_events, instance_health_checks, drift_detections.';
