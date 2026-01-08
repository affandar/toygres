-- 0004_add_last_health_check.sql
-- Description: Add last_health_check column to track when health was last checked

SET search_path TO toygres_cms, public;

-- Add the column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'toygres_cms'
          AND table_name = 'instances'
          AND column_name = 'last_health_check'
    ) THEN
        ALTER TABLE instances ADD COLUMN last_health_check TIMESTAMPTZ;
    END IF;
END;
$$;
