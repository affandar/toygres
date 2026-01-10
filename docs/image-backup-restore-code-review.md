# Code Review Summary: Image Backup & Restore Feature

## Overview

This PR implements PostgreSQL image backup and restore functionality for Toygres, allowing users to:
1. Create point-in-time backups (images) of PostgreSQL instances
2. Restore new instances from existing backup images
3. Gracefully handle instance actor lifecycle during deletion

**Files Changed:** 34 files, +3,684 lines, -40 lines

---

## Major Features

### 1. Image Backup System

**Flow:**
1. User requests backup via `POST /api/instances/:name/images`
2. `create_image` orchestration runs:
   - Creates CMS record for image
   - Runs K8s Job with `pg_dump` to create backup
   - Uploads backup to Azure Blob Storage via `azcopy`
   - Updates image status to "available"

**New Files:**
- `toygres-orchestrations/src/orchestrations/create_image.rs` - Backup orchestration
- `toygres-orchestrations/src/activities/run_backup_job.rs` - Runs pg_dump + azcopy upload
- `toygres-orchestrations/src/templates/backup-job.yaml` - K8s Job template for backup

### 2. Instance Restore from Image

**Flow:**
1. User creates instance with `source_image_id` parameter
2. `create_instance` v1.0.2 orchestration:
   - Fetches source image metadata (image_type, postgres_version)
   - Creates empty PVC
   - Retrieves source password from image record
   - Runs restore job (azcopy download + pg_restore)
   - Deploys PostgreSQL using existing PVC
   - Optionally changes password

**New Files:**
- `toygres-orchestrations/src/activities/run_restore_job.rs` - Runs azcopy download + pg_restore
- `toygres-orchestrations/src/activities/deploy_postgres_from_pvc.rs` - Deploy using existing PVC
- `toygres-orchestrations/src/activities/create_pvc.rs` - Create empty PVC
- `toygres-orchestrations/src/activities/get_instance_password.rs` - Get password from K8s Secret
- `toygres-orchestrations/src/templates/restore-job.yaml` - K8s Job template for restore

### 3. Instance Actor Graceful Shutdown

**Problem Solved:** Instance actors were failing with timeout errors when instances were deleted because they couldn't find the CMS record.

**Solution:** Delete orchestration v1.0.2 sends "InstanceDeleted" external event to actor before cleanup.

**New Files:**
- `toygres-orchestrations/src/activities/send_external_event.rs` - Sends external events to orchestrations

---

## Key Changes by Component

### API Server (`toygres-server/src/api.rs`)

| Change | Description |
|--------|-------------|
| Image routes | Added `/api/images`, `/api/images/:name`, `/api/instances/:name/images` |
| CreateInstanceRequest | Added `source_image_id` field for restore |
| start_orchestration | Changed from versioned to unversioned (uses Latest policy) |
| cancel_orchestration | Implemented using `cancel_instance()` |

### Orchestrations

| File | Version | Changes |
|------|---------|---------|
| `create_instance.rs` | v1.0.2 | Added restore from image support, image_type preservation |
| `delete_instance.rs` | v1.0.2 | Sends InstanceDeleted signal to actor before cleanup |
| `create_image.rs` | New | Backup orchestration |

### Activity Types (`activity_types.rs`)

New types added:
- `ImageOperation` enum - CMS operations for images
- `ImageOperationResult` enum - Results including `PasswordFound`
- `RunBackupJobInput/Output` - Backup job parameters
- `RunRestoreJobInput/Output` - Restore job parameters
- `CreatePvcInput/Output` - PVC creation
- `DeployPostgresFromPvcInput/Output` - Deploy with existing PVC
- `SendExternalEventInput/Output` - External event signaling

### K8s Templates

| Template | Purpose |
|----------|---------|
| `backup-job.yaml` | pg_dump + azcopy upload to blob |
| `restore-job.yaml` | azcopy download + pg_restore |
| `postgres-config.yaml` | ConfigMap with pg_hba.conf (localhost trust for pg_durable) |
| `postgres-secret.yaml` | Secret template for password |

### Database Migrations

| Migration | Purpose |
|-----------|---------|
| `0005_move_enums_to_cms_schema.sql` | Move enums to cms schema |
| `0006_add_images.sql` | Add images table with blob_url, source_password_encrypted |

---

## Important Implementation Details

### 1. azcopy Workload Identity

```yaml
# CORRECT - Uses AKS workload identity
azcopy login --login-type=workload

# WRONG - Uses IMDS, fails with 403
azcopy login --identity
```

### 2. pg_hba.conf for pg_durable

pg_durable background worker connects via TCP (127.0.0.1), not Unix socket:

```
# Must use trust for localhost TCP connections
host    all    all    127.0.0.1/32    trust
host    all    all    ::1/128         trust
```

### 3. DNS Propagation Timeout

Azure LoadBalancer DNS propagation can take 60-90+ seconds. `test_connection` timeout increased from 60s to 120s.

### 4. Image Type Preservation

When restoring, the orchestration fetches source image details BEFORE creating CMS record to preserve the correct `image_type` (stock vs pg_durable).

### 5. Password Handling

Restore uses the source image's encrypted password (stored in `source_password_encrypted` column), not a new password provided by the user.

---

## UI Changes

| Component | Changes |
|-----------|---------|
| `App.tsx` | Added Images route |
| `Sidebar.tsx` | Added Images navigation link |
| `api.ts` | Added image API functions |
| `types.ts` | Added Image interface |
| `components/images/` | New image list and detail components |

---

## Skills Documentation

New/updated Claude Code skills:
- `image-backup-restore/SKILL.md` - New skill for backup/restore debugging
- `aks-deployment/SKILL.md` - Added DNS propagation timing info
- `duroxide-orchestrations/SKILL.md` - Added versioning patterns

---

## Testing Performed

1. **Local testing:**
   - Created instance, created backup, restored from backup
   - Verified pg_durable background worker runs correctly
   - Verified instance actor graceful shutdown

2. **AKS testing:**
   - Deployed to AKS cluster
   - Created instance (akstest1) - verified v1.0.2 orchestration
   - Deleted instance - verified actor received InstanceDeleted signal
   - Actor completed gracefully (not failed with timeout)

---

## Risk Areas

1. **azcopy authentication** - Workload identity must be configured correctly
2. **Blob storage permissions** - Service account needs Storage Blob Data Contributor role
3. **Password encryption** - Currently stored as UTF-8 bytes (not encrypted), needs proper encryption
4. **Large backups** - No size limits or progress tracking for large databases

---

## Future Improvements

1. Add proper password encryption for `source_password_encrypted`
2. Add backup progress tracking and ETA
3. Add backup size limits and validation
4. Add scheduled/automated backups
5. Add backup retention policies
