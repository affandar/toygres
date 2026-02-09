---
name: image-backup-restore
description: Implementing and debugging PostgreSQL image backup and restore features. Use when working with database snapshots, backup jobs, restore operations, azcopy blob transfers, or troubleshooting image-related provisioning failures.
---

# PostgreSQL Image Backup & Restore

## Overview

Image backup creates point-in-time snapshots of PostgreSQL instances by:
1. Running pg_dump to create a logical backup
2. Uploading to Azure Blob Storage via azcopy
3. Recording metadata in CMS database

Image restore provisions new instances from existing backups by:
1. Creating PVC, Secret, ConfigMap for the new instance
2. Running azcopy download + pg_restore to load data from blob storage
3. Starting PostgreSQL with the restored data

## Key Files

```
toygres-orchestrations/src/
├── orchestrations/
│   ├── create_image.rs        # Backup orchestration
│   └── create_instance.rs     # Restore flow (v1.0.5 inlined)
├── activities/
│   ├── run_backup_job.rs      # pg_dump + azcopy upload
│   ├── run_restore_job.rs     # azcopy download + pg_restore
│   ├── deploy_postgres_from_pvc.rs  # Start from existing PVC
│   ├── create_pvc.rs          # Create empty PVC for restore
│   └── cms/
│       └── image_ops.rs       # Image CMS operations
├── templates/
│   ├── backup-job.yaml        # K8s Job for backup
│   └── restore-job.yaml       # K8s Job for restore
└── activity_types.rs          # ImageOperation enum
```

## Backup Flow

```rust
// create_image.rs orchestration
1. CreateImageRecord        // Reserve ID in CMS
2. GetInstanceByK8sName     // Get source instance details
3. RunBackupJob             // pg_dump + azcopy to blob
4. UpdateImageStatus        // Mark as available
```

## Restore Flow (v1.0.5)

The restore logic is now **inlined** in create_instance v1.0.5 (no delegation to older versions):

```rust
// create_instance.rs v1.0.5 - create_from_image_impl()
1. Fetch image details (validate state = 'ready')
2. Get source password from image record
3. CreatePvc                // Create empty PVC
4. RunRestoreJob            // azcopy download + extract
5. Wait for restore job completion
6. DeployPostgresFromPvc    // Start PostgreSQL
7. Wait for pod ready
8. GetConnectionStrings
9. TestConnection           // Verify connectivity
```

### Critical: Password Handling

The restore flow must use the **source image's password**, not a new password:

```rust
// Fetch from CMS image_ops activity
let password_result = ctx
    .schedule_activity_typed::<ImageOperation, ImageOperationResult>(
        cms::image_ops::NAME,
        &ImageOperation::GetSourcePassword { id: image_uuid },
    )
    .await?;
```

### Critical: Image Type Preservation

When restoring, the image_type is fetched from the source image record:

```rust
let image_type = match image.image_type.as_str() {
    "stock" => ImageType::Stock,
    "pg_durable" => ImageType::PgDurable,
    _ => ImageType::Stock,
};
```

This ensures pg_durable images are deployed with correct configuration.

## azcopy Workload Identity

**CRITICAL:** On AKS, always use `--login-type=workload`:

```bash
# WRONG - Uses IMDS (VM managed identity), fails with 403
azcopy login --identity

# CORRECT - Uses federated token from pod
azcopy login --login-type=workload
```

Debug workload identity issues:

```bash
# Check env vars are injected
kubectl exec <pod> -- env | grep AZURE_

# Should see:
# AZURE_CLIENT_ID=...
# AZURE_TENANT_ID=...
# AZURE_FEDERATED_TOKEN_FILE=/var/run/secrets/azure/tokens/azure-identity-token
```

## Common Issues

### test_connection Timeout

**Symptom:** Instance provisioning fails at test_connection step.

**Root cause:** Azure LoadBalancer DNS propagation can take 60-90+ seconds.

**Fix:** Use 120s timeout:

```rust
RetryPolicy::new(5)
    .with_timeout(Duration::from_secs(120)), // Not 60s!
```

### azcopy 403 AuthorizationPermissionMismatch

**Symptom:** `azcopy login --identity` succeeds but operations fail with 403.

**Root cause:** `--identity` uses IMDS, not AKS workload identity.

**Fix:** Use `--login-type=workload`.

### Restore Shows Wrong image_type

**Symptom:** Restored instance shows "stock" instead of "pg_durable" in CMS.

**Root cause:** Using input.image_type instead of source image's image_type.

**Fix:** Fetch source image details before creating CMS record.

## Debugging Commands

```bash
# Watch backup/restore job logs
kubectl logs -n toygres job/<job-name> -f

# Check blob storage
az storage blob list --account-name <acct> --container-name images --auth-mode login

# Test PostgreSQL connection
kubectl exec -it <postgres-pod> -n toygres -- psql -U postgres -c "SELECT 1"

# View job completion status
kubectl get jobs -n toygres
```

## API Endpoints

```bash
# Create backup image
POST /api/images
{
    "name": "my-backup",
    "instance_id": "uuid",
    "description": "Optional description"
}

# Create instance from image
POST /api/instances
{
    "name": "restored-instance",
    "password": "ignored-uses-source",
    "source_image_id": "image-uuid",
    "dns_label": "restored-instance"
}
```

Note: `source_image_id` cannot be combined with `runtime_image_id` or `image_override`.

