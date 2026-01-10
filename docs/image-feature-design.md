# Image Feature Design Spec

## Overview

This feature allows users to create reusable "images" from running PostgreSQL instances using `pg_basebackup`. Images can be used to deploy new instances with the same data, extensions, users, and configuration.

### Goals
- **Zero downtime** image creation (pg_basebackup streams from running instance)
- **100% portable** (blob storage, works on any K8s)
- **Complete capture** of database state (all DBs, users, extensions, config)
- **Simple restore** with new name/DNS/password

### Non-Goals (v1)
- Incremental backups (full backup each time)
- Point-in-time recovery (PITR)
- Scheduled/automatic image creation
- Cross-region replication

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              USER FLOW                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  [Instance Detail Page]                    [Create Instance Page]            │
│         │                                          │                         │
│         ▼                                          ▼                         │
│  "Create Image" button              "From Image" dropdown                    │
│         │                                          │                         │
│         ▼                                          ▼                         │
│  POST /api/instances/:name/images   POST /api/instances (with source_image) │
│         │                                          │                         │
└─────────┼──────────────────────────────────────────┼─────────────────────────┘
          │                                          │
          ▼                                          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ORCHESTRATIONS                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  CreateImageOrchestration:                 CreateInstanceOrchestration:      │
│  ─────────────────────────                 (modified)                        │
│  1. Validate source instance               ────────────────────────────      │
│  2. Create CMS record (creating)           If source_image provided:         │
│  3. Run backup Job in K8s                    1. Create PVC (empty)           │
│     └─ pg_basebackup → blob                  2. Run restore Job              │
│  4. Wait for Job completion                     └─ blob → data dir           │
│  5. Update CMS (ready)                       3. Create StatefulSet           │
│                                              4. Wait for ready               │
│  DeleteImageOrchestration:                   5. Change password (SQL)        │
│  ─────────────────────────                   6. Update CMS                   │
│  1. Delete blob from storage                                                 │
│  2. Delete CMS record                                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
          │                                          │
          ▼                                          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                             ACTIVITIES                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  K8s Activities (new):                     CMS Activity (new):               │
│  ─────────────────────                     ───────────────────               │
│  • run_backup_job                          • image_ops (consolidated)        │
│  • wait_for_job                              └─ Create                       │
│  • delete_job                                └─ UpdateState                  │
│  • run_restore_job                           └─ Get                          │
│  • delete_blob                               └─ List                         │
│  • change_password                           └─ Delete                       │
│                                                                              │
│  Existing (modified):                                                        │
│  ────────────────────                                                        │
│  • deploy_postgres      (skip PVC if restoring from image)                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           BLOB STORAGE                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Azure Blob Storage Container: toygres-images                               │
│  ─────────────────────────────────────────────                              │
│  images/                                                                     │
│  ├── prod-snapshot-jan/                                                     │
│  │   ├── base.tar.gz           (pg_basebackup output)                       │
│  │   └── metadata.json         (checksum, size, timestamp)                  │
│  ├── staging-baseline/                                                      │
│  │   ├── base.tar.gz                                                        │
│  │   └── metadata.json                                                      │
│  └── ...                                                                    │
│                                                                              │
│  Access: Managed Identity (AKS → Storage Account)                           │
│  Or: SAS tokens / connection string in Secret                               │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Database Schema

### New Migration: `0003_images.sql`

```sql
-- Image state enum (in toygres_cms schema, not public)
SET search_path TO toygres_cms;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_type t
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

-- Images table
CREATE TABLE IF NOT EXISTS toygres_cms.images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- User-facing metadata
    name VARCHAR(255) UNIQUE NOT NULL,
    description TEXT,
    
    -- Source instance reference
    source_instance_id UUID REFERENCES toygres_cms.instances(id) ON DELETE SET NULL,
    source_k8s_name VARCHAR(255) NOT NULL,
    source_namespace VARCHAR(255) NOT NULL DEFAULT 'toygres',
    
    -- Backup storage location
    blob_storage_url TEXT NOT NULL,          -- e.g., https://account.blob.core.windows.net/toygres-images/prod-snapshot/
    blob_container VARCHAR(255) NOT NULL,    -- e.g., toygres-images
    blob_path VARCHAR(512) NOT NULL,         -- e.g., images/prod-snapshot-jan/
    
    -- Inherited configuration (for restore)
    storage_size_gb INTEGER NOT NULL,
    postgres_version VARCHAR(50) NOT NULL,
    image_type VARCHAR(50) NOT NULL DEFAULT 'stock',  -- 'stock' or 'pg_durable'
    
    -- Password handling: encrypted source password for restore
    -- Encrypted using server-side key (from env or Key Vault)
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
    ready_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- Partial unique index for active images (replaces inline constraint)
CREATE UNIQUE INDEX IF NOT EXISTS idx_images_unique_active_name 
    ON toygres_cms.images(name) 
    WHERE state != 'deleted';

CREATE INDEX IF NOT EXISTS idx_images_name ON toygres_cms.images(name);
CREATE INDEX IF NOT EXISTS idx_images_state ON toygres_cms.images(state);
CREATE INDEX IF NOT EXISTS idx_images_source_instance ON toygres_cms.images(source_instance_id);

-- Add source_image reference to instances table
ALTER TABLE toygres_cms.instances 
    ADD COLUMN IF NOT EXISTS source_image_id UUID REFERENCES toygres_cms.images(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_instances_source_image ON toygres_cms.instances(source_image_id);
```

---

## Activities

### New K8s Activities

#### 1. `run_backup_job`

Creates a K8s Job that runs pg_basebackup and uploads to blob storage.

```rust
// toygres-orchestrations/src/activities/run_backup_job.rs

pub const NAME: &str = "toygres-orchestrations::activity::run-backup-job";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBackupJobInput {
    pub job_name: String,
    pub namespace: String,
    pub source_instance_name: String,
    pub source_password: String,
    pub blob_storage_url: String,
    pub blob_container: String,
    pub blob_path: String,
    pub image_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBackupJobOutput {
    pub job_name: String,
    pub created: bool,
}

pub async fn activity(
    ctx: ActivityContext,
    input: RunBackupJobInput,
) -> Result<RunBackupJobOutput, String> {
    // Create Job that:
    // 1. Runs pg_basebackup -h <source>-svc -U postgres -D /backup -Ft -z -X stream
    // 2. Uploads /backup/base.tar.gz to blob storage using azcopy or az cli
}
```

**Job Template** (`templates/backup-job.yaml`):

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: {{ job_name }}
  namespace: {{ namespace }}
  labels:
    app: toygres-backup
    image: {{ image_name }}
spec:
  ttlSecondsAfterFinished: 3600  # Cleanup after 1 hour
  backoffLimit: 2
  template:
    spec:
      restartPolicy: Never
      containers:
      - name: backup
        image: toygresacr.azurecr.io/toygres-backup:latest  # Custom image with pg_basebackup + azcopy
        env:
        - name: PGHOST
          value: "{{ source_instance_name }}-svc"
        - name: PGUSER
          value: "postgres"
        - name: PGPASSWORD
          valueFrom:
            secretKeyRef:
              name: {{ job_name }}-creds
              key: password
        - name: BLOB_STORAGE_URL
          value: "{{ blob_storage_url }}"
        - name: BLOB_CONTAINER
          value: "{{ blob_container }}"
        - name: BLOB_PATH
          value: "{{ blob_path }}"
        - name: AZURE_STORAGE_CONNECTION_STRING
          valueFrom:
            secretKeyRef:
              name: toygres-blob-storage
              key: connection-string
        command:
        - /bin/sh
        - -c
        - |
          set -e
          echo "Starting pg_basebackup..."
          pg_basebackup -h $PGHOST -U $PGUSER -D /backup -Ft -z -X stream -P
          
          echo "Calculating checksum..."
          sha256sum /backup/base.tar.gz > /backup/checksum.txt
          
          echo "Uploading to blob storage..."
          azcopy copy "/backup/base.tar.gz" "${BLOB_STORAGE_URL}/${BLOB_CONTAINER}/${BLOB_PATH}/base.tar.gz"
          azcopy copy "/backup/checksum.txt" "${BLOB_STORAGE_URL}/${BLOB_CONTAINER}/${BLOB_PATH}/checksum.txt"
          
          echo "Backup complete!"
        volumeMounts:
        - name: backup-vol
          mountPath: /backup
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "1"
      volumes:
      - name: backup-vol
        emptyDir:
          sizeLimit: 100Gi  # Adjust based on max DB size
```

#### 2. `wait_for_job`

Polls Job status until completion or failure.

```rust
// toygres-orchestrations/src/activities/wait_for_job.rs

pub const NAME: &str = "toygres-orchestrations::activity::wait-for-job";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForJobInput {
    pub job_name: String,
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForJobOutput {
    pub succeeded: bool,
    pub failed: bool,
    pub active: bool,
    pub completion_time: Option<String>,
}

pub async fn activity(
    ctx: ActivityContext,
    input: WaitForJobInput,
) -> Result<WaitForJobOutput, String> {
    // Check Job status using K8s API
    // Return current state (caller will poll via orchestration timer)
}
```

#### 3. `run_restore_job`

Creates a K8s Job that downloads backup and restores to a PVC.

```rust
// toygres-orchestrations/src/activities/run_restore_job.rs

pub const NAME: &str = "toygres-orchestrations::activity::run-restore-job";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRestoreJobInput {
    pub job_name: String,
    pub namespace: String,
    pub target_pvc_name: String,
    pub blob_storage_url: String,
    pub blob_container: String,
    pub blob_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRestoreJobOutput {
    pub job_name: String,
    pub created: bool,
}
```

**Job Template** (`templates/restore-job.yaml`):

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: {{ job_name }}
  namespace: {{ namespace }}
  labels:
    app: toygres-restore
spec:
  ttlSecondsAfterFinished: 3600
  backoffLimit: 2
  template:
    spec:
      restartPolicy: Never
      containers:
      - name: restore
        image: toygresacr.azurecr.io/toygres-backup:latest
        env:
        - name: BLOB_STORAGE_URL
          value: "{{ blob_storage_url }}"
        - name: BLOB_CONTAINER
          value: "{{ blob_container }}"
        - name: BLOB_PATH
          value: "{{ blob_path }}"
        - name: AZURE_STORAGE_CONNECTION_STRING
          valueFrom:
            secretKeyRef:
              name: toygres-blob-storage
              key: connection-string
        command:
        - /bin/sh
        - -c
        - |
          set -e
          echo "Downloading backup from blob storage..."
          azcopy copy "${BLOB_STORAGE_URL}/${BLOB_CONTAINER}/${BLOB_PATH}/base.tar.gz" /backup/base.tar.gz
          
          echo "Verifying checksum..."
          azcopy copy "${BLOB_STORAGE_URL}/${BLOB_CONTAINER}/${BLOB_PATH}/checksum.txt" /backup/checksum.txt
          cd /backup && sha256sum -c checksum.txt
          
          echo "Extracting to data directory..."
          mkdir -p /data/pgdata
          tar -xzf /backup/base.tar.gz -C /data/pgdata
          
          # Fix permissions for postgres user (UID 999 in official image)
          chown -R 999:999 /data/pgdata
          chmod 700 /data/pgdata
          
          echo "Restore complete!"
        volumeMounts:
        - name: backup-vol
          mountPath: /backup
        - name: data-vol
          mountPath: /data
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "1"
      volumes:
      - name: backup-vol
        emptyDir:
          sizeLimit: 100Gi
      - name: data-vol
        persistentVolumeClaim:
          claimName: {{ target_pvc_name }}
```

#### 4. `delete_blob`

Deletes backup files from blob storage.

```rust
// toygres-orchestrations/src/activities/delete_blob.rs

pub const NAME: &str = "toygres-orchestrations::activity::delete-blob";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBlobInput {
    pub blob_storage_url: String,
    pub blob_container: String,
    pub blob_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBlobOutput {
    pub deleted: bool,
}
```

#### 5. `change_password`

Connects to PostgreSQL and changes the password via SQL.

```rust
// toygres-orchestrations/src/activities/change_password.rs

pub const NAME: &str = "toygres-orchestrations::activity::change-password";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordInput {
    pub host: String,          // e.g., "new-instance-svc.toygres.svc.cluster.local"
    pub port: u16,             // 5432
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordOutput {
    pub changed: bool,
}

pub async fn activity(
    ctx: ActivityContext,
    input: ChangePasswordInput,
) -> Result<ChangePasswordOutput, String> {
    // Connect with current_password
    // Execute: ALTER USER postgres PASSWORD '<new_password>';
    // Verify by reconnecting with new_password
}
```

#### 6. `delete_job`

Cleans up K8s Job and associated secrets.

```rust
// toygres-orchestrations/src/activities/delete_job.rs

pub const NAME: &str = "toygres-orchestrations::activity::delete-job";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteJobInput {
    pub job_name: String,
    pub namespace: String,
}
```

### CMS Activity: Consolidated Image Operations

Instead of separate activities for each CMS operation, we use a **single activity with typed enum operations**.
This reduces boilerplate while maintaining type safety and idempotency.

```rust
// toygres-orchestrations/src/activities/cms/image_ops.rs

pub const NAME: &str = "toygres-orchestrations::activity::cms-image-ops";

// ============================================================================
// Operation Enum - All image CMS operations in one place
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ImageOperation {
    /// Create a new image record (idempotent via ON CONFLICT)
    Create {
        name: String,
        description: Option<String>,
        source_instance_id: Uuid,
        source_k8s_name: String,
        source_namespace: String,
        blob_storage_url: String,
        blob_container: String,
        blob_path: String,
        storage_size_gb: i32,
        postgres_version: String,
        image_type: String,
        source_password_encrypted: Vec<u8>,
        orchestration_id: String,
    },
    
    /// Update image state (creating → ready, failed, deleting, deleted)
    UpdateState {
        name: String,
        state: String,
        backup_size_bytes: Option<i64>,
        backup_checksum: Option<String>,
        error_message: Option<String>,
    },
    
    /// Get image by name
    Get {
        name: String,
    },
    
    /// List all images (optionally filtered by state)
    List {
        state_filter: Option<String>,  // None = all non-deleted
    },
    
    /// Soft-delete image record
    Delete {
        name: String,
    },
}

// ============================================================================
// Result Enum - Type-safe responses
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ImageResult {
    /// Single image record
    Image(ImageRecord),
    
    /// List of images
    ImageList(Vec<ImageRecord>),
    
    /// Mutation result
    Modified { 
        affected: u64,
        image_id: Option<Uuid>,
    },
    
    /// Image not found
    NotFound { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRecord {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub source_instance_id: Option<Uuid>,
    pub source_k8s_name: String,
    pub blob_storage_url: String,
    pub blob_container: String,
    pub blob_path: String,
    pub storage_size_gb: i32,
    pub postgres_version: String,
    pub image_type: String,
    pub source_password_encrypted: Vec<u8>,
    pub backup_size_bytes: Option<i64>,
    pub backup_checksum: Option<String>,
    pub state: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub ready_at: Option<String>,
}

// ============================================================================
// Activity Implementation
// ============================================================================

pub async fn activity(
    ctx: ActivityContext,
    op: ImageOperation,
) -> Result<ImageResult, String> {
    let pool = get_pool().await?;
    
    match op {
        ImageOperation::Create { name, description, source_instance_id, ... } => {
            ctx.trace_info(format!("CMS: Creating image record '{}'", name));
            
            // Idempotent insert with ON CONFLICT
            let result = sqlx::query(r#"
                INSERT INTO toygres_cms.images 
                (name, description, source_instance_id, source_k8s_name, ...)
                VALUES ($1, $2, $3, ...)
                ON CONFLICT (name) DO UPDATE
                SET description = EXCLUDED.description,
                    updated_at = NOW()
                WHERE images.create_orchestration_id = EXCLUDED.create_orchestration_id
                RETURNING id
            "#)
            .bind(&name)
            // ... bind all params
            .fetch_one(&pool)
            .await
            .map_err(|e| format!("Failed to create image record: {}", e))?;
            
            let id: Uuid = result.try_get("id")?;
            Ok(ImageResult::Modified { affected: 1, image_id: Some(id) })
        }
        
        ImageOperation::UpdateState { name, state, backup_size_bytes, ... } => {
            ctx.trace_info(format!("CMS: Updating image '{}' state to '{}'", name, state));
            
            let result = sqlx::query(r#"
                UPDATE toygres_cms.images
                SET state = $2::image_state,
                    backup_size_bytes = COALESCE($3, backup_size_bytes),
                    backup_checksum = COALESCE($4, backup_checksum),
                    error_message = $5,
                    ready_at = CASE WHEN $2 = 'ready' THEN NOW() ELSE ready_at END,
                    deleted_at = CASE WHEN $2 = 'deleted' THEN NOW() ELSE deleted_at END
                WHERE name = $1
            "#)
            .bind(&name)
            .bind(&state)
            .bind(backup_size_bytes)
            .bind(backup_checksum)
            .bind(error_message)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to update image state: {}", e))?;
            
            Ok(ImageResult::Modified { affected: result.rows_affected(), image_id: None })
        }
        
        ImageOperation::Get { name } => {
            ctx.trace_info(format!("CMS: Getting image '{}'", name));
            
            let row = sqlx::query_as::<_, ImageRecord>(r#"
                SELECT * FROM toygres_cms.images
                WHERE name = $1 AND state != 'deleted'
            "#)
            .bind(&name)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to get image: {}", e))?;
            
            match row {
                Some(record) => Ok(ImageResult::Image(record)),
                None => Ok(ImageResult::NotFound { name }),
            }
        }
        
        ImageOperation::List { state_filter } => {
            ctx.trace_info("CMS: Listing images");
            
            let images = sqlx::query_as::<_, ImageRecord>(r#"
                SELECT * FROM toygres_cms.images
                WHERE state != 'deleted'
                  AND ($1::text IS NULL OR state = $1::image_state)
                ORDER BY created_at DESC
            "#)
            .bind(state_filter)
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Failed to list images: {}", e))?;
            
            Ok(ImageResult::ImageList(images))
        }
        
        ImageOperation::Delete { name } => {
            ctx.trace_info(format!("CMS: Soft-deleting image '{}'", name));
            
            let result = sqlx::query(r#"
                UPDATE toygres_cms.images
                SET state = 'deleted', deleted_at = NOW()
                WHERE name = $1
            "#)
            .bind(&name)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete image: {}", e))?;
            
            Ok(ImageResult::Modified { affected: result.rows_affected(), image_id: None })
        }
    }
}
```

### Usage in Orchestrations

```rust
// Creating an image record
let result = ctx.schedule_activity_typed(
    cms::image_ops::NAME,
    &ImageOperation::Create {
        name: "prod-snapshot".to_string(),
        description: Some("Production backup".to_string()),
        source_instance_id: instance.id,
        // ... other fields
    }
).into_activity_typed::<ImageResult>().await?;

let image_id = match result {
    ImageResult::Modified { image_id: Some(id), .. } => id,
    _ => return Err("Failed to create image record".to_string()),
};

// Getting an image
let result = ctx.schedule_activity_typed(
    cms::image_ops::NAME,
    &ImageOperation::Get { name: "prod-snapshot".to_string() }
).into_activity_typed::<ImageResult>().await?;

let image = match result {
    ImageResult::Image(img) => img,
    ImageResult::NotFound { name } => return Err(format!("Image '{}' not found", name)),
    _ => return Err("Unexpected result type".to_string()),
};

// Updating state
ctx.schedule_activity_typed(
    cms::image_ops::NAME,
    &ImageOperation::UpdateState {
        name: "prod-snapshot".to_string(),
        state: "ready".to_string(),
        backup_size_bytes: Some(1024 * 1024 * 500),
        backup_checksum: Some("sha256:abc123...".to_string()),
        error_message: None,
    }
).into_activity_typed::<ImageResult>().await?;
```

### Benefits of This Approach

| Aspect | Separate Activities | Consolidated Enum |
|--------|--------------------|--------------------|
| Files | 4+ files | 1 file |
| Registration | 4+ register calls | 1 register call |
| Type safety | ✅ Separate structs | ✅ Enum variants |
| Idempotency | Per-file logic | Centralized |
| Traces | `cms-create-image-record` | `cms-image-ops: Create(prod-snapshot)` |
| Adding operations | New file + registration | Add enum variant + match arm |

---

## Orchestrations

### 1. `CreateImageOrchestration`

```rust
// toygres-orchestrations/src/orchestrations/create_image.rs

pub const NAME: &str = "toygres-orchestrations::orchestration::create-image";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateImageInput {
    pub image_name: String,
    pub description: Option<String>,
    pub source_instance_name: String,  // k8s_name
    pub orchestration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateImageOutput {
    pub image_name: String,
    pub blob_path: String,
    pub backup_size_bytes: i64,
}

pub async fn create_image_orchestration(
    ctx: OrchestrationContext,
    input: CreateImageInput,
) -> Result<CreateImageOutput, String> {
    ctx.trace_info(format!("Creating image '{}' from instance '{}'", 
        input.image_name, input.source_instance_name));
    
    // Step 1: Get source instance details from CMS
    let instance = ctx.schedule_activity_typed(
        cms::get_instance::NAME,
        &GetInstanceInput { k8s_name: input.source_instance_name.clone() }
    ).into_activity_typed().await?;
    
    if instance.state != "running" {
        return Err(format!("Instance must be running, current state: {}", instance.state));
    }
    
    // Step 2: Encrypt source password
    let encrypted_password = encrypt_password(&instance.password)?;
    
    // Step 3: Create CMS record using consolidated image_ops activity
    let blob_path = format!("images/{}/", input.image_name);
    let blob_storage_url = std::env::var("BLOB_STORAGE_URL")
        .unwrap_or_else(|_| "https://toygresacr.blob.core.windows.net".to_string());
    let blob_container = std::env::var("BLOB_CONTAINER")
        .unwrap_or_else(|_| "toygres-images".to_string());
    
    let create_result = ctx.schedule_activity_typed(
        cms::image_ops::NAME,
        &ImageOperation::Create {
            name: input.image_name.clone(),
            description: input.description,
            source_instance_id: instance.id,
            source_k8s_name: instance.k8s_name.clone(),
            source_namespace: instance.namespace.clone(),
            blob_storage_url: blob_storage_url.clone(),
            blob_container: blob_container.clone(),
            blob_path: blob_path.clone(),
            storage_size_gb: instance.storage_size_gb,
            postgres_version: instance.postgres_version.clone(),
            image_type: instance.image_type.clone(),
            source_password_encrypted: encrypted_password,
            orchestration_id: input.orchestration_id.clone(),
        }
    ).into_activity_typed::<ImageResult>().await?;
    
    let _image_id = match create_result {
        ImageResult::Modified { image_id: Some(id), .. } => id,
        _ => return Err("Failed to create image record".to_string()),
    };
    
    // Step 4: Run backup job
    let job_name = format!("backup-{}", input.image_name);
    ctx.schedule_activity_typed(
        run_backup_job::NAME,
        &RunBackupJobInput {
            job_name: job_name.clone(),
            namespace: instance.namespace.clone(),
            source_instance_name: instance.k8s_name.clone(),
            source_password: instance.password.clone(),
            blob_storage_url: blob_storage_url.clone(),
            blob_container: blob_container.clone(),
            blob_path: blob_path.clone(),
            image_name: input.image_name.clone(),
        }
    ).into_activity_typed().await?;
    
    // Step 5: Wait for job completion (poll with timers)
    let max_attempts = 120;  // 20 minutes (120 * 10s)
    for attempt in 1..=max_attempts {
        let job_status = ctx.schedule_activity_typed(
            wait_for_job::NAME,
            &WaitForJobInput {
                job_name: job_name.clone(),
                namespace: instance.namespace.clone(),
            }
        ).into_activity_typed::<WaitForJobOutput>().await?;
        
        if job_status.succeeded {
            ctx.trace_info("Backup job completed successfully");
            break;
        }
        
        if job_status.failed {
            // Update CMS to failed state
            ctx.schedule_activity_typed(
                cms::image_ops::NAME,
                &ImageOperation::UpdateState {
                    name: input.image_name.clone(),
                    state: "failed".to_string(),
                    backup_size_bytes: None,
                    backup_checksum: None,
                    error_message: Some("Backup job failed".to_string()),
                }
            ).into_activity_typed::<ImageResult>().await?;
            
            return Err("Backup job failed".to_string());
        }
        
        if attempt >= max_attempts {
            return Err("Backup job timed out".to_string());
        }
        
        ctx.schedule_timer(Duration::from_secs(10)).into_timer().await;
    }
    
    // Step 6: Get backup metadata (size, checksum) - could read from blob or job logs
    // For now, update as ready without size
    ctx.schedule_activity_typed(
        cms::image_ops::NAME,
        &ImageOperation::UpdateState {
            name: input.image_name.clone(),
            state: "ready".to_string(),
            backup_size_bytes: None,  // TODO: get from blob metadata
            backup_checksum: None,    // TODO: get from checksum.txt
            error_message: None,
        }
    ).into_activity_typed::<ImageResult>().await?;
    
    // Step 7: Cleanup job
    let _ = ctx.schedule_activity_typed(
        delete_job::NAME,
        &DeleteJobInput {
            job_name: job_name,
            namespace: instance.namespace,
        }
    ).into_activity_typed().await;
    
    ctx.trace_info(format!("Image '{}' created successfully", input.image_name));
    
    Ok(CreateImageOutput {
        image_name: input.image_name,
        blob_path,
        backup_size_bytes: 0,  // TODO
    })
}
```

### 2. `DeleteImageOrchestration`

```rust
// toygres-orchestrations/src/orchestrations/delete_image.rs

pub const NAME: &str = "toygres-orchestrations::orchestration::delete-image";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteImageInput {
    pub image_name: String,
    pub orchestration_id: String,
}

pub async fn delete_image_orchestration(
    ctx: OrchestrationContext,
    input: DeleteImageInput,
) -> Result<(), String> {
    ctx.trace_info(format!("Deleting image '{}'", input.image_name));
    
    // Step 1: Get image details
    let image_result = ctx.schedule_activity_typed(
        cms::image_ops::NAME,
        &ImageOperation::Get { name: input.image_name.clone() }
    ).into_activity_typed::<ImageResult>().await?;
    
    let image = match image_result {
        ImageResult::Image(img) => img,
        ImageResult::NotFound { name } => return Err(format!("Image '{}' not found", name)),
        _ => return Err("Unexpected result from Get operation".to_string()),
    };
    
    // Step 2: Mark as deleting
    ctx.schedule_activity_typed(
        cms::image_ops::NAME,
        &ImageOperation::UpdateState {
            name: input.image_name.clone(),
            state: "deleting".to_string(),
            backup_size_bytes: None,
            backup_checksum: None,
            error_message: None,
        }
    ).into_activity_typed::<ImageResult>().await?;
    
    // Step 3: Delete blob storage files
    ctx.schedule_activity_typed(
        delete_blob::NAME,
        &DeleteBlobInput {
            blob_storage_url: image.blob_storage_url,
            blob_container: image.blob_container,
            blob_path: image.blob_path,
        }
    ).into_activity_typed().await?;
    
    // Step 4: Mark as deleted (soft delete)
    ctx.schedule_activity_typed(
        cms::image_ops::NAME,
        &ImageOperation::Delete { name: input.image_name.clone() }
    ).into_activity_typed::<ImageResult>().await?;
    
    ctx.trace_info(format!("Image '{}' deleted", input.image_name));
    Ok(())
}
```

### 3. Modified `CreateInstanceOrchestration`

Add support for `source_image` parameter:

```rust
// Additions to CreateInstanceInput
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceInput {
    // ... existing fields ...
    
    /// Optional: Create from existing image instead of empty
    pub source_image: Option<String>,
}

// In create_instance_orchestration:
pub async fn create_instance_orchestration(
    ctx: OrchestrationContext,
    input: CreateInstanceInput,
) -> Result<CreateInstanceOutput, String> {
    
    // ... existing validation ...
    
    let (image_type, postgres_version, storage_size_gb, source_password, image_record) = 
        if let Some(ref image_name) = input.source_image {
            // Get image metadata using consolidated image_ops activity
            let image_result = ctx.schedule_activity_typed(
                cms::image_ops::NAME,
                &ImageOperation::Get { name: image_name.clone() }
            ).into_activity_typed::<ImageResult>().await?;
            
            let image = match image_result {
                ImageResult::Image(img) => img,
                ImageResult::NotFound { name } => {
                    return Err(format!("Image '{}' not found", name))
                },
                _ => return Err("Unexpected result from Get operation".to_string()),
            };
            
            if image.state != "ready" {
                return Err(format!("Image '{}' is not ready (state: {})", image_name, image.state));
            }
            
            // Storage must be >= image storage
            let storage = input.storage_size_gb.unwrap_or(image.storage_size_gb);
            if storage < image.storage_size_gb {
                return Err(format!(
                    "Storage size {} GB is less than image size {} GB",
                    storage, image.storage_size_gb
                ));
            }
            
            let source_pwd = decrypt_password(&image.source_password_encrypted)?;
            
            (
                ImageType::from_str(&image.image_type),
                image.postgres_version.clone(),
                storage,
                Some(source_pwd),
                Some(image),
            )
        } else {
            // Existing behavior - use input params
            (
                input.image_type.clone(),
                input.postgres_version.clone().unwrap_or_else(|| "17".to_string()),
                input.storage_size_gb.unwrap_or(10),
                None,
                None,
            )
        };
    
    // Create CMS record (with source_image_id if applicable)
    // ...
    
    if let (Some(image), Some(source_pwd)) = (image_record, source_password) {
        // RESTORE PATH
        create_instance_from_image(&ctx, &input, &image, source_pwd).await
    } else {
        // EXISTING PATH
        create_instance_empty(&ctx, &input).await
    }
}

async fn create_instance_from_image(
    ctx: &OrchestrationContext,
    input: &CreateInstanceInput,
    image: &ImageRecord,  // Using ImageRecord from image_ops
    source_password: String,
) -> Result<CreateInstanceOutput, String> {
    let namespace = input.namespace.clone().unwrap_or_else(|| "toygres".to_string());
    
    // Step 1: Create empty PVC
    ctx.trace_info("Step 1: Creating PVC for restored data");
    ctx.schedule_activity_typed(
        create_pvc::NAME,  // New activity - just creates PVC
        &CreatePvcInput {
            namespace: namespace.clone(),
            pvc_name: format!("{}-pvc", input.name),
            storage_size_gb: image.storage_size_gb,
        }
    ).into_activity_typed().await?;
    
    // Step 2: Run restore job
    ctx.trace_info("Step 2: Restoring data from image");
    let job_name = format!("restore-{}", input.name);
    ctx.schedule_activity_typed(
        run_restore_job::NAME,
        &RunRestoreJobInput {
            job_name: job_name.clone(),
            namespace: namespace.clone(),
            target_pvc_name: format!("{}-pvc", input.name),
            blob_storage_url: image.blob_storage_url.clone(),
            blob_container: image.blob_container.clone(),
            blob_path: image.blob_path.clone(),
        }
    ).into_activity_typed().await?;
    
    // Step 3: Wait for restore job
    ctx.trace_info("Step 3: Waiting for restore to complete");
    // ... similar polling logic as backup ...
    
    // Step 4: Deploy StatefulSet + Service (no PVC creation - already exists)
    ctx.trace_info("Step 4: Deploying PostgreSQL");
    ctx.schedule_activity_typed(
        deploy_postgres::NAME,
        &DeployPostgresInput {
            namespace: namespace.clone(),
            instance_name: input.name.clone(),
            password: input.password.clone(),  // This is ignored by postgres (data exists)
            postgres_version: image.postgres_version.clone(),
            storage_size_gb: image.storage_size_gb,
            use_load_balancer: input.use_load_balancer.unwrap_or(true),
            dns_label: input.dns_label.clone(),
            image_type: ImageType::from_str(&image.image_type),
            skip_pvc_creation: true,  // NEW FLAG - PVC already created
        }
    ).into_activity_typed().await?;
    
    // Step 5: Wait for pod ready
    ctx.trace_info("Step 5: Waiting for pod to be ready");
    // ... existing wait_for_ready logic ...
    
    // Step 6: Change password
    ctx.trace_info("Step 6: Changing password from source to new");
    ctx.schedule_activity_typed(
        change_password::NAME,
        &ChangePasswordInput {
            host: format!("{}-svc.{}.svc.cluster.local", input.name, namespace),
            port: 5432,
            current_password: source_password,
            new_password: input.password.clone(),
        }
    ).into_activity_typed().await?;
    
    // Step 7: Get connection strings, cleanup, etc.
    // ... existing logic ...
}
```

---

## API Endpoints

### New Endpoints

```rust
// toygres-server/src/api.rs

// Create image from instance
// POST /api/instances/:name/images
#[derive(Debug, Deserialize)]
struct CreateImageRequest {
    image_name: String,
    description: Option<String>,
}

async fn create_image(
    State(state): State<AppState>,
    Path(instance_name): Path<String>,
    Json(req): Json<CreateImageRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate image name
    // Start CreateImageOrchestration
    // Return orchestration ID
}

// List all images
// GET /api/images
async fn list_images(
    State(state): State<AppState>,
) -> Result<Json<Vec<ImageSummary>>, AppError> {
    // Query CMS for all images where state != 'deleted'
}

// Get image details
// GET /api/images/:name
async fn get_image(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ImageDetail>, AppError> {
    // Query CMS for image by name
}

// Delete image
// DELETE /api/images/:name
async fn delete_image(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Start DeleteImageOrchestration
}

// Modified: Create instance with optional source_image
// POST /api/instances
#[derive(Debug, Deserialize)]
struct CreateInstanceRequest {
    // ... existing fields ...
    
    /// Optional: Image name to create from
    source_image: Option<String>,
}
```

### Route Registration

```rust
// Add to create_router()
.route("/api/instances/:name/images", post(create_image))
.route("/api/images", get(list_images))
.route("/api/images/:name", get(get_image).delete(delete_image))
```

---

## UI Changes

### 1. Instance Detail Page

Add "Create Image" button:

```tsx
// components/instances/InstanceDetail.tsx

<Button
  onClick={() => setShowCreateImageModal(true)}
  disabled={instance.state !== 'running'}
>
  <Camera className="h-4 w-4 mr-2" />
  Create Image
</Button>

{showCreateImageModal && (
  <CreateImageModal
    instanceName={instance.k8s_name}
    onClose={() => setShowCreateImageModal(false)}
    onSuccess={() => {
      showToast('success', 'Image creation started');
      setShowCreateImageModal(false);
    }}
  />
)}
```

### 2. Create Instance Form

Add "From Image" option:

```tsx
// components/instances/CreateInstance.tsx

const [sourceType, setSourceType] = useState<'empty' | 'image'>('empty');
const [selectedImage, setSelectedImage] = useState<string>('');

// Fetch available images
const { data: images } = useQuery({
  queryKey: ['images'],
  queryFn: () => api.listImages(),
});

// In form:
<div className="space-y-3">
  <label>Start From</label>
  <RadioGroup value={sourceType} onValueChange={setSourceType}>
    <RadioGroupItem value="empty">Empty Database</RadioGroupItem>
    <RadioGroupItem value="image">From Image</RadioGroupItem>
  </RadioGroup>
</div>

{sourceType === 'image' && (
  <div className="space-y-2">
    <label>Select Image</label>
    <Select value={selectedImage} onValueChange={setSelectedImage}>
      {images?.map(img => (
        <SelectItem key={img.name} value={img.name}>
          {img.name} ({img.storage_size_gb} GB, {img.image_type})
        </SelectItem>
      ))}
    </Select>
    <p className="text-xs text-muted">
      PostgreSQL version and image type are inherited from the source image.
    </p>
  </div>
)}

// Disable version/image_type selects when sourceType === 'image'
```

### 3. New Images Page

```tsx
// components/images/ImageList.tsx

export function ImageList() {
  const { data: images, isLoading } = useQuery({
    queryKey: ['images'],
    queryFn: () => api.listImages(),
  });

  return (
    <div className="space-y-6">
      <div>
        <h1>Images</h1>
        <p>Snapshots of PostgreSQL instances</p>
      </div>

      <div className="grid gap-4">
        {images?.map(image => (
          <ImageCard key={image.id} image={image} />
        ))}
      </div>
    </div>
  );
}
```

### 4. Navigation

Add Images link to sidebar:

```tsx
// components/layout/Sidebar.tsx

<NavItem href="/images" icon={<Database />}>
  Images
</NavItem>
```

---

## Infrastructure Setup

### 1. Azure Blob Storage

```bash
# Create storage account (if not exists)
az storage account create \
  --name toygresstorage \
  --resource-group toygres-rg \
  --location westus3 \
  --sku Standard_LRS

# Create container
az storage container create \
  --name toygres-images \
  --account-name toygresstorage

# Get connection string
az storage account show-connection-string \
  --name toygresstorage \
  --resource-group toygres-rg \
  --query connectionString -o tsv
```

### 2. K8s Secret

```yaml
# deploy/k8s/blob-storage-secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: toygres-blob-storage
  namespace: toygres-system
type: Opaque
stringData:
  connection-string: "DefaultEndpointsProtocol=https;AccountName=..."
```

### 3. Backup Container Image

```dockerfile
# deploy/Dockerfile.backup
FROM postgres:17

# Install azcopy
RUN apt-get update && apt-get install -y curl && \
    curl -L https://aka.ms/downloadazcopy-v10-linux | tar xz --strip-components=1 -C /usr/local/bin && \
    chmod +x /usr/local/bin/azcopy

# No entrypoint - will be specified in Job
```

---

## Implementation Order

### Phase 1: Foundation
1. [ ] Database migration (`0003_images.sql` - images table + source_image_id on instances)
2. [ ] Blob storage setup (Azure Storage Account + K8s secret)
3. [ ] Build `toygres-backup` Docker image (postgres + azcopy)
4. [ ] Add `TOYGRES_ENCRYPTION_KEY` env var for password encryption

### Phase 2: Activities
5. [ ] `cms/image_ops.rs` - Consolidated CMS activity with enum operations
6. [ ] `run_backup_job.rs` - Create K8s Job for pg_basebackup
7. [ ] `wait_for_job.rs` - Poll Job status
8. [ ] `delete_job.rs` - Cleanup Job + secrets
9. [ ] `delete_blob.rs` - Remove backup from blob storage
10. [ ] `change_password.rs` - ALTER USER via SQL
11. [ ] `create_pvc.rs` - Create empty PVC (for restore path)

### Phase 3: Create Image Flow
12. [ ] `CreateImageOrchestration`
13. [ ] `DeleteImageOrchestration`
14. [ ] API endpoints: `POST /api/instances/:name/images`, `GET/DELETE /api/images/:name`
15. [ ] UI: Images list page (`/images`)
16. [ ] UI: Create Image modal on Instance Detail page

### Phase 4: Restore from Image Flow
17. [ ] `run_restore_job.rs` - Create K8s Job for restore
18. [ ] Modify `deploy_postgres.rs` for `skip_pvc_creation` flag
19. [ ] Modify `CreateInstanceOrchestration` for `source_image` parameter
20. [ ] API: Add `source_image` field to create instance endpoint
21. [ ] UI: "From Image" option in Create Instance form

### Phase 5: Polish
22. [ ] Error handling and rollback on failures
23. [ ] Backup size/checksum tracking from blob metadata
24. [ ] Progress indicators in UI during backup/restore
25. [ ] Testing on local kind cluster
26. [ ] Documentation

---

## Security Considerations

### Password Encryption

```rust
// toygres-orchestrations/src/crypto.rs

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

/// Encryption key derived from TOYGRES_ENCRYPTION_KEY env var
/// In production, use Azure Key Vault
fn get_encryption_key() -> Key<Aes256Gcm> {
    let key_str = std::env::var("TOYGRES_ENCRYPTION_KEY")
        .expect("TOYGRES_ENCRYPTION_KEY must be set");
    // Derive 256-bit key using HKDF or similar
}

pub fn encrypt_password(password: &str) -> Result<Vec<u8>, String> {
    let key = get_encryption_key();
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(b"unique nonce"); // Should be random per encryption
    cipher.encrypt(nonce, password.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))
}

pub fn decrypt_password(encrypted: &[u8]) -> Result<String, String> {
    let key = get_encryption_key();
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(b"unique nonce");
    let decrypted = cipher.decrypt(nonce, encrypted)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    String::from_utf8(decrypted)
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}
```

### Blob Storage Access

- Use Managed Identity where possible (AKS → Storage Account)
- Fallback to SAS tokens with limited scope
- Connection strings stored in K8s Secrets

---

## Error Handling

### Backup Failures

```rust
// In CreateImageOrchestration
if job_status.failed {
    ctx.trace_error("Backup job failed, marking image as failed");
    
    // Update CMS
    update_image_state("failed", "Backup job failed").await?;
    
    // Cleanup any partial uploads
    let _ = delete_blob(&blob_path).await;
    
    // Cleanup job
    let _ = delete_job(&job_name).await;
    
    return Err("Backup failed".to_string());
}
```

### Restore Failures

```rust
// In create_instance_from_image
match restore_and_deploy().await {
    Ok(output) => Ok(output),
    Err(e) => {
        ctx.trace_error(format!("Restore failed: {}", e));
        
        // Cleanup PVC
        let _ = delete_pvc(&pvc_name).await;
        
        // Cleanup any K8s resources
        let _ = delete_postgres(&instance_name).await;
        
        // Update CMS to failed
        update_instance_state("failed", &e).await;
        
        Err(e)
    }
}
```

---

## Testing

### Unit Tests
- Activity input/output serialization
- Password encryption/decryption
- CMS query idempotency

### Integration Tests
- Full backup/restore cycle on local K8s (kind)
- Verify data integrity after restore
- Password change verification

### Manual Testing
1. Create instance with sample data
2. Create image from instance
3. Create new instance from image
4. Verify data matches
5. Verify password is changed
6. Delete image, verify blob cleanup

---

## Future Enhancements

- **Incremental backups** using pgBackRest
- **Scheduled images** (daily/weekly)
- **Cross-region copy** for DR
- **Image retention policies** (auto-delete after N days)
- **Image sharing** between users/tenants
- **Compression levels** (speed vs size tradeoff)
