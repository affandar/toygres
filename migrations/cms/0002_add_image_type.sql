-- 0002_add_image_type.sql
-- Description: Add image_type column to support stock PostgreSQL vs pg_durable images

SET search_path TO toygres_cms, public;

-- Add image_type enum
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typname = 'image_type' AND n.nspname = 'toygres_cms'
    ) THEN
        CREATE TYPE toygres_cms.image_type AS ENUM ('stock', 'pg_durable');
    END IF;
END;
$$;

-- Add image_type column to instances table
ALTER TABLE instances 
ADD COLUMN IF NOT EXISTS image_type toygres_cms.image_type NOT NULL DEFAULT 'stock';

-- Create index for filtering by image type
CREATE INDEX IF NOT EXISTS idx_instances_image_type ON instances(image_type);

COMMENT ON COLUMN instances.image_type IS 'Type of PostgreSQL image: stock (vanilla postgres) or pg_durable (with duroxide extension)';

