-- 0006_add_images.sql
-- Description: Add images table for pg_basebackup snapshots feature
-- Allows users to create images from instances and deploy new instances from images

SET search_path TO toygres_cms;

-- ============================================================================
-- Image state enum
-- ============================================================================

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'image_state' AND n.nspname = 'toygres_cms'
    ) THEN
        CREATE TYPE toygres_cms.image_state AS ENUM (
            'creating',    -- Backup in progress
            'ready',       -- Available for use
            'failed',      -- Backup failed
            'deleting',    -- Deletion in progress
            'deleted'      -- Soft-deleted
        );
    END IF;
END;
$$;

-- ============================================================================
-- Images table
-- ============================================================================

CREATE TABLE IF NOT EXISTS toygres_cms.images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- User-facing metadata
    name VARCHAR(255) NOT NULL,
    description TEXT,
    
    -- Source instance reference (nullable - instance may be deleted)
    source_instance_id UUID REFERENCES toygres_cms.instances(id) ON DELETE SET NULL,
    source_k8s_name VARCHAR(255) NOT NULL,
    source_namespace VARCHAR(255) NOT NULL DEFAULT 'toygres',
    
    -- Backup storage location
    blob_storage_url TEXT NOT NULL,          -- e.g., https://account.blob.core.windows.net
    blob_container VARCHAR(255) NOT NULL,    -- e.g., toygres-images
    blob_path VARCHAR(512) NOT NULL,         -- e.g., images/prod-snapshot-jan/
    
    -- Inherited configuration (for restore validation)
    storage_size_gb INTEGER NOT NULL,
    postgres_version VARCHAR(50) NOT NULL,
    image_type toygres_cms.image_type NOT NULL DEFAULT 'stock',
    
    -- Password handling: encrypted source password for restore
    -- Encrypted using server-side key (TOYGRES_ENCRYPTION_KEY env or Key Vault)
    source_password_encrypted BYTEA,
    
    -- Backup metadata
    backup_size_bytes BIGINT,
    backup_checksum VARCHAR(128),            -- SHA256 of backup file
    
    -- Orchestration tracking
    state toygres_cms.image_state NOT NULL DEFAULT 'creating',
    create_orchestration_id TEXT NOT NULL,
    error_message TEXT,
    
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ready_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- Partial unique index for active images (allows same name after deletion)
CREATE UNIQUE INDEX IF NOT EXISTS idx_images_unique_active_name 
    ON toygres_cms.images(name) 
    WHERE state != 'deleted';

CREATE INDEX IF NOT EXISTS idx_images_name ON toygres_cms.images(name);
CREATE INDEX IF NOT EXISTS idx_images_state ON toygres_cms.images(state);
CREATE INDEX IF NOT EXISTS idx_images_source_instance ON toygres_cms.images(source_instance_id);
CREATE INDEX IF NOT EXISTS idx_images_created_at ON toygres_cms.images(created_at DESC);

-- Trigger for updated_at
DROP TRIGGER IF EXISTS update_images_updated_at ON toygres_cms.images;
CREATE TRIGGER update_images_updated_at
    BEFORE UPDATE ON toygres_cms.images
    FOR EACH ROW
    EXECUTE FUNCTION toygres_cms.update_updated_at_column();

-- ============================================================================
-- Add source_image reference to instances table
-- ============================================================================

ALTER TABLE toygres_cms.instances 
    ADD COLUMN IF NOT EXISTS source_image_id UUID REFERENCES toygres_cms.images(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_instances_source_image ON toygres_cms.instances(source_image_id);

COMMENT ON COLUMN toygres_cms.instances.source_image_id IS 'Reference to the image this instance was created from (NULL if created empty)';

-- ============================================================================
-- Image events table (for audit trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS toygres_cms.image_events (
    id BIGSERIAL PRIMARY KEY,
    image_id UUID NOT NULL REFERENCES toygres_cms.images(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,        -- 'state_change', 'backup_started', 'backup_completed', etc.
    old_state VARCHAR(50),
    new_state VARCHAR(50),
    message TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_image_events_image_id ON toygres_cms.image_events(image_id);
CREATE INDEX IF NOT EXISTS idx_image_events_type ON toygres_cms.image_events(event_type);
CREATE INDEX IF NOT EXISTS idx_image_events_created_at ON toygres_cms.image_events(created_at DESC);

-- ============================================================================
-- Comments
-- ============================================================================

COMMENT ON TABLE toygres_cms.images IS 'PostgreSQL instance images created via pg_basebackup, stored in blob storage';
COMMENT ON TABLE toygres_cms.image_events IS 'Audit trail of image lifecycle events';
