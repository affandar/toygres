-- 0007_add_runtime_images.sql
-- Description: Add runtime_images catalog (ACR container images) and instance runtime_image reference

SET search_path TO toygres_cms;

-- ============================================================================
-- Runtime images table
-- ============================================================================

CREATE TABLE IF NOT EXISTS toygres_cms.runtime_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- User-facing metadata
    name VARCHAR(255) NOT NULL,
    description TEXT,

    -- Must be in the Toygres ACR; store separately + allow canonical pull ref
    acr_ref TEXT NOT NULL,          -- e.g. toygresacr.azurecr.io/myrepo:tag or toygresacr.azurecr.io/myrepo
    digest VARCHAR(128) NOT NULL,   -- sha256:...

    -- Suggested behavior for deployments (stock vs pg_durable env conventions)
    suggested_image_type toygres_cms.image_type NOT NULL DEFAULT 'stock',

    -- Lifecycle (keep simple for phase 1)
    state TEXT NOT NULL DEFAULT 'ready',  -- ready|deleted
    error_message TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

-- Partial unique index for active runtime images
CREATE UNIQUE INDEX IF NOT EXISTS idx_runtime_images_unique_active_name
    ON toygres_cms.runtime_images(name)
    WHERE state != 'deleted';

CREATE INDEX IF NOT EXISTS idx_runtime_images_name ON toygres_cms.runtime_images(name);
CREATE INDEX IF NOT EXISTS idx_runtime_images_state ON toygres_cms.runtime_images(state);
CREATE INDEX IF NOT EXISTS idx_runtime_images_created_at ON toygres_cms.runtime_images(created_at DESC);

-- Trigger for updated_at
DROP TRIGGER IF EXISTS update_runtime_images_updated_at ON toygres_cms.runtime_images;
CREATE TRIGGER update_runtime_images_updated_at
    BEFORE UPDATE ON toygres_cms.runtime_images
    FOR EACH ROW
    EXECUTE FUNCTION toygres_cms.update_updated_at_column();

COMMENT ON TABLE toygres_cms.runtime_images IS 'Catalog of operator-registered OCI images in Toygres ACR usable for deployments';

-- ============================================================================
-- Instance references runtime image
-- ============================================================================

ALTER TABLE toygres_cms.instances
    ADD COLUMN IF NOT EXISTS runtime_image_id UUID REFERENCES toygres_cms.runtime_images(id) ON DELETE RESTRICT;

CREATE INDEX IF NOT EXISTS idx_instances_runtime_image ON toygres_cms.instances(runtime_image_id);

COMMENT ON COLUMN toygres_cms.instances.runtime_image_id IS 'Optional reference to runtime image (ACR OCI) used to deploy this instance';
