---
name: database-changes
description: Making database schema changes to the CMS database. Use when adding columns, tables, running migrations, or updating the backend API and TypeScript types for new database fields.
---

# Database Schema Changes

## Overview
End-to-end process for adding new columns or tables to the CMS database.

## Adding a New Column

### 1. Create Migration
```sql
-- migrations/cms/0004_add_my_column.sql
SET search_path TO toygres_cms, public;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'toygres_cms'
          AND table_name = 'instances'
          AND column_name = 'my_column'
    ) THEN
        ALTER TABLE instances ADD COLUMN my_column VARCHAR(255);
    END IF;
END;
$$;
```

### 2. Run Migration
```bash
./scripts/db-migrate.sh
```

### 3. Update Backend API
In `toygres-server/src/api.rs`, add to SELECT query:
```rust
let row = sqlx::query(
    "SELECT ..., my_column FROM toygres_cms.instances WHERE ..."
)
```

Add to JSON response:
```rust
Ok(Json(serde_json::json!({
    // existing fields...
    "my_column": row.get::<Option<String>, _>("my_column")
})))
```

### 4. Update TypeScript Types
In `toygres-ui/src/lib/types.ts`:
```typescript
export interface InstanceDetail extends Instance {
  // existing fields...
  my_column: string | null;
}
```

### 5. Update Activities (if needed)
If an activity should set this column:
```rust
sqlx::query("UPDATE toygres_cms.instances SET my_column = $2 WHERE k8s_name = $1")
    .bind(&input.k8s_name)
    .bind(&input.my_column)
    .execute(&pool)
    .await?;
```

## Adding a New Table (Catalog Entity Pattern)

Example: Adding a `runtime_images` table for ACR image catalog.

### 1. Create Migration
```sql
-- migrations/cms/0007_add_runtime_images.sql
SET search_path TO toygres_cms, public;

CREATE TABLE IF NOT EXISTS runtime_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    acr_ref VARCHAR(512) NOT NULL,
    digest VARCHAR(128) NOT NULL,
    suggested_image_type VARCHAR(32) NOT NULL DEFAULT 'stock',
    state VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 2. Add API Endpoints
```rust
// In api.rs

// List endpoint
async fn list_runtime_images(State(state): State<AppState>) -> Result<Json<Vec<...>>, AppError> {
    let rows = sqlx::query("SELECT * FROM toygres_cms.runtime_images WHERE state != 'deleted' ORDER BY created_at DESC")
        .fetch_all(&*state.pool).await?;
    // ...
}

// Create/Register endpoint
async fn register_runtime_image(State(state): State<AppState>, Json(req): Json<...>) -> Result<Json<...>, AppError> {
    // Validate, insert, return
}

// Add routes
.route("/api/runtime-images", get(list_runtime_images))
.route("/api/runtime-images/register", post(register_runtime_image))
```

### 3. Add TypeScript Types
```typescript
// types.ts
export interface RuntimeImage {
  id: string;
  name: string;
  description: string | null;
  acr_ref: string;
  digest: string;
  suggested_image_type: string;
  created_at: string;
}
```

### 4. Add API Functions
```typescript
// api.ts
async listRuntimeImages() {
  return fetchJson<RuntimeImage[]>(`${API_BASE}/api/runtime-images`);
}

async registerRuntimeImage(data: { name: string; acr_ref: string; digest: string; }) {
  return fetchJson(`${API_BASE}/api/runtime-images/register`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}
```

### 5. Add UI Component
Create a component with list view and registration form.

## Idempotency Patterns

All CMS activities must be idempotent for Duroxide replay safety:

```sql
-- Upsert pattern
INSERT INTO table (id, value) VALUES ($1, $2)
ON CONFLICT (id) DO UPDATE SET value = $2;

-- Conditional update
UPDATE table SET state = 'new_state'
WHERE id = $1 AND state = 'expected_state';
```

