# PostgreSQL Backup Plan - Automated Periodic Backups for Toygres

## Overview

Implement automated, periodic backups for all PostgreSQL instances deployed by Toygres. Backups will be stored in Azure Blob Storage with configurable retention policies, supporting both full database dumps and point-in-time recovery (PITR).

---

## Backup Strategy

### Types of Backups

#### 1. **Logical Backups** (pg_dump)
- **What:** SQL dump of database contents
- **Size:** Compressed ~30-50% of data size
- **Speed:** Slower (must read all data)
- **Restore:** Flexible (can restore individual tables, databases)
- **Use case:** Small-to-medium databases, selective restore

#### 2. **Physical Backups** (pg_basebackup)
- **What:** Binary copy of data directory
- **Size:** Same as data directory size
- **Speed:** Faster (file-level copy)
- **Restore:** Full cluster restore only
- **Use case:** Large databases, PITR

#### 3. **WAL Archiving** (Continuous)
- **What:** Transaction logs (Write-Ahead Logs)
- **Size:** Depends on write volume (~1-100 MB per file)
- **Speed:** Real-time streaming
- **Restore:** Point-in-time recovery with basebackup
- **Use case:** Zero data loss, recover to any point

### Our Approach

**Hybrid Strategy:**
1. **Daily logical backups** (pg_dump) - Easy restore, good for most use cases
2. **Weekly physical backups** (pg_basebackup) - Full cluster snapshot
3. **Continuous WAL archiving** (optional) - For PITR capability

---

## Architecture Design

### Storage Location

**Azure Blob Storage** (Hot tier for recent, Cool tier for old backups)

```
Azure Storage Account: toygresstorage
├── Container: toygres-backups
    ├── mydb-abc123/
    │   ├── daily/
    │   │   ├── 2025-11-17-daily.sql.gz
    │   │   ├── 2025-11-16-daily.sql.gz
    │   │   └── ...
    │   ├── weekly/
    │   │   ├── 2025-11-15-weekly.tar.gz
    │   │   ├── 2025-11-08-weekly.tar.gz
    │   │   └── ...
    │   └── wal/  (optional)
    │       ├── 000000010000000000000001
    │       └── ...
    └── foodb-def456/
        └── ...
```

### Kubernetes Resources

#### CronJob for Daily Backups

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: {{ instance_name }}-daily-backup
  namespace: {{ namespace }}
spec:
  schedule: "0 2 * * *"  # 2 AM daily
  successfulJobsHistoryLimit: 3
  failedJobsHistoryLimit: 3
  jobTemplate:
    spec:
      template:
        spec:
          restartPolicy: OnFailure
          containers:
          - name: backup
            image: postgres:{{ postgres_version }}
            command:
            - bash
            - -c
            - |
              set -e
              
              # Timestamp for filename
              TIMESTAMP=$(date +%Y-%m-%d-%H%M%S)
              BACKUP_FILE="daily-${TIMESTAMP}.sql.gz"
              
              echo "Starting daily backup: ${BACKUP_FILE}"
              
              # Run pg_dump
              PGPASSWORD=${POSTGRES_PASSWORD} pg_dump \
                -h {{ instance_name }} \
                -U postgres \
                -Fc \
                -f /tmp/backup.dump \
                postgres
              
              # Compress
              gzip /tmp/backup.dump
              
              # Upload to Azure Blob Storage
              az storage blob upload \
                --account-name toygresstorage \
                --container-name toygres-backups \
                --name {{ instance_name }}/daily/${BACKUP_FILE} \
                --file /tmp/backup.dump.gz \
                --auth-mode login
              
              echo "Backup completed: ${BACKUP_FILE}"
              
              # Clean up old backups (retention policy)
              python3 /scripts/cleanup_old_backups.py \
                --instance {{ instance_name }} \
                --type daily \
                --keep-days 7
            env:
            - name: POSTGRES_PASSWORD
              valueFrom:
                secretKeyRef:
                  name: {{ instance_name }}-credentials
                  key: password
            - name: AZURE_STORAGE_ACCOUNT
              value: toygresstorage
            volumeMounts:
            - name: backup-scripts
              mountPath: /scripts
          volumes:
          - name: backup-scripts
            configMap:
              name: backup-scripts
          serviceAccountName: toygres-backup
```

#### CronJob for Weekly Backups

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: {{ instance_name }}-weekly-backup
spec:
  schedule: "0 1 * * 0"  # 1 AM every Sunday
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: backup
            command:
            - bash
            - -c
            - |
              # Use pg_basebackup for physical backup
              TIMESTAMP=$(date +%Y-%m-%d-%H%M%S)
              BACKUP_FILE="weekly-${TIMESTAMP}.tar.gz"
              
              PGPASSWORD=${POSTGRES_PASSWORD} pg_basebackup \
                -h {{ instance_name }} \
                -U postgres \
                -D /tmp/basebackup \
                -Ft \
                -z \
                -P
              
              # Upload to Azure
              az storage blob upload \
                --account-name toygresstorage \
                --container-name toygres-backups \
                --name {{ instance_name }}/weekly/${BACKUP_FILE} \
                --file /tmp/basebackup.tar.gz
              
              # Retention: keep 4 weeks
              python3 /scripts/cleanup_old_backups.py \
                --instance {{ instance_name }} \
                --type weekly \
                --keep-weeks 4
```

---

## Backup Retention Policy

### Default Retention

| Backup Type | Frequency | Retention | Storage Tier | Purpose |
|-------------|-----------|-----------|--------------|---------|
| Daily (logical) | Every day at 2 AM | 7 days | Hot | Quick recovery, recent data |
| Weekly (physical) | Sunday at 1 AM | 4 weeks | Hot | Full restore, compliance |
| Monthly (logical) | 1st of month | 3 months | Cool | Long-term archive |
| Quarterly (physical) | Every 3 months | 1 year | Cool | Compliance, audit |

### Configurable Per Instance

```bash
# Create with custom retention
toygres create mydb --password secret \
  --backup-daily-retention 14 \
  --backup-weekly-retention 8 \
  --backup-monthly-retention 6
```

---

## CMS Database Schema Changes

### Extend `instances` Table

```sql
ALTER TABLE toygres_cms.instances 
ADD COLUMN IF NOT EXISTS backup_enabled BOOLEAN NOT NULL DEFAULT true,
ADD COLUMN IF NOT EXISTS backup_schedule_daily VARCHAR(50) DEFAULT '0 2 * * *',  -- cron expression
ADD COLUMN IF NOT EXISTS backup_schedule_weekly VARCHAR(50) DEFAULT '0 1 * * 0',
ADD COLUMN IF NOT EXISTS backup_retention_days INTEGER DEFAULT 7,
ADD COLUMN IF NOT EXISTS backup_retention_weeks INTEGER DEFAULT 4,
ADD COLUMN IF NOT EXISTS last_backup_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS last_backup_status VARCHAR(50),  -- 'success', 'failed', 'in_progress'
ADD COLUMN IF NOT EXISTS last_backup_size_bytes BIGINT,
ADD COLUMN IF NOT EXISTS backup_storage_path TEXT;  -- Azure blob path

CREATE INDEX IF NOT EXISTS idx_instances_backup_enabled 
    ON instances(backup_enabled) 
    WHERE backup_enabled = true;
```

### New Table: `backups`

```sql
CREATE TABLE IF NOT EXISTS toygres_cms.backups (
    id BIGSERIAL PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    backup_type VARCHAR(50) NOT NULL,  -- 'daily', 'weekly', 'monthly', 'manual'
    backup_method VARCHAR(50) NOT NULL,  -- 'pg_dump', 'pg_basebackup', 'wal'
    storage_path TEXT NOT NULL,  -- Azure blob path
    size_bytes BIGINT,
    status VARCHAR(50) NOT NULL,  -- 'in_progress', 'completed', 'failed'
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    
    CONSTRAINT check_status CHECK (status IN ('in_progress', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_backups_instance_id ON backups(instance_id);
CREATE INDEX IF NOT EXISTS idx_backups_type ON backups(backup_type);
CREATE INDEX IF NOT EXISTS idx_backups_status ON backups(status);
CREATE INDEX IF NOT EXISTS idx_backups_started_at ON backups(started_at DESC);

-- Unique constraint: only one in-progress backup per instance at a time
CREATE UNIQUE INDEX IF NOT EXISTS idx_backups_in_progress 
    ON backups(instance_id) 
    WHERE status = 'in_progress';
```

---

## New Activities

### 1. `create_backup_cronjob`

**Purpose**: Create Kubernetes CronJob for automated backups

**Input:**
```rust
pub struct CreateBackupCronJobInput {
    pub instance_name: String,
    pub namespace: String,
    pub backup_type: String,  // "daily" or "weekly"
    pub schedule: String,      // Cron expression
    pub retention_days: i32,
    pub postgres_version: String,
}
```

**Steps:**
1. Load CronJob template
2. Render with instance details
3. Create CronJob in Kubernetes
4. Verify CronJob created successfully

**Output:**
```rust
pub struct CreateBackupCronJobOutput {
    pub cronjob_name: String,
    pub schedule: String,
    pub next_run: String,  // Calculated next execution time
}
```

### 2. `delete_backup_cronjob`

**Purpose**: Remove backup CronJob when instance is deleted

**Input:**
```rust
pub struct DeleteBackupCronJobInput {
    pub instance_name: String,
    pub namespace: String,
    pub backup_type: String,  // "daily" or "weekly"
}
```

**Steps:**
1. Delete CronJob from Kubernetes
2. Optionally: Delete associated backup files from Azure
3. Update CMS database

### 3. `trigger_manual_backup`

**Purpose**: Run immediate backup (doesn't wait for schedule)

**Input:**
```rust
pub struct TriggerManualBackupInput {
    pub instance_name: String,
    pub namespace: String,
    pub backup_method: String,  // "pg_dump" or "pg_basebackup"
    pub label: String,           // e.g., "pre-migration", "before-upgrade"
}
```

**Steps:**
1. Create Kubernetes Job (one-time) from CronJob template
2. Wait for job completion
3. Record backup in CMS database
4. Return backup details

**Output:**
```rust
pub struct TriggerManualBackupOutput {
    pub backup_id: i64,
    pub storage_path: String,
    pub size_bytes: i64,
    pub duration_seconds: i32,
}
```

### 4. `list_backups`

**Purpose**: List available backups for an instance

**Input:**
```rust
pub struct ListBackupsInput {
    pub instance_name: String,
    pub backup_type: Option<String>,  // Filter by type
    pub limit: i32,
}
```

**Steps:**
1. Query CMS database for backup records
2. Optionally: Query Azure Blob Storage for files
3. Return sorted list (newest first)

**Output:**
```rust
pub struct ListBackupsOutput {
    pub backups: Vec<BackupInfo>,
}

pub struct BackupInfo {
    pub id: i64,
    pub backup_type: String,
    pub storage_path: String,
    pub size_bytes: i64,
    pub created_at: String,
    pub status: String,
}
```

### 5. `restore_from_backup`

**Purpose**: Restore PostgreSQL instance from backup

**Input:**
```rust
pub struct RestoreFromBackupInput {
    pub instance_name: String,
    pub backup_id: i64,
    pub namespace: String,
    pub new_instance_name: Option<String>,  // Restore to new instance
}
```

**Steps:**
1. Lookup backup details from CMS
2. Stop target PostgreSQL instance (if restoring in-place)
3. Download backup from Azure Blob Storage
4. Restore backup:
   - If pg_dump: `pg_restore` or `psql`
   - If pg_basebackup: Extract and replace data directory
5. Start PostgreSQL instance
6. Verify restoration successful
7. Record restore event in CMS

**Output:**
```rust
pub struct RestoreFromBackupOutput {
    pub restored_to_instance: String,
    pub backup_timestamp: String,
    pub restore_duration_seconds: i32,
}
```

### 6. `verify_backup`

**Purpose**: Test backup integrity (can it be restored?)

**Input:**
```rust
pub struct VerifyBackupInput {
    pub backup_id: i64,
}
```

**Steps:**
1. Create temporary pod
2. Download backup file
3. Attempt restore to temp location
4. Run basic SQL queries
5. Clean up temp resources
6. Record verification result

**Output:**
```rust
pub struct VerifyBackupOutput {
    pub valid: bool,
    pub error_message: Option<String>,
    pub verification_time_seconds: i32,
}
```

---

## New Orchestrations

### `BackupInstanceOrchestration` (NEW)

**Purpose**: Perform manual backup of an instance

**Trigger**: User command or scheduled

**Workflow:**
```rust
pub async fn backup_instance_orchestration(
    ctx: OrchestrationContext,
    input: BackupInstanceInput,
) -> Result<BackupInstanceOutput, String> {
    ctx.trace_info(format!("Starting backup for instance: {}", input.instance_name));
    
    // Step 1: Record backup start in CMS
    let backup_record = create_backup_record(...).await?;
    
    // Step 2: Trigger backup job
    let backup_result = trigger_manual_backup(...).await?;
    
    // Step 3: Update CMS with backup completion
    update_backup_record(
        backup_id: backup_record.id,
        status: "completed",
        size_bytes: backup_result.size_bytes,
        storage_path: backup_result.storage_path,
    ).await?;
    
    // Step 4: Update instance last_backup_at
    update_instance_backup_status(...).await?;
    
    ctx.trace_info("Backup completed successfully");
    
    Ok(BackupInstanceOutput {
        backup_id: backup_record.id,
        storage_path: backup_result.storage_path,
        size_bytes: backup_result.size_bytes,
    })
}
```

### `RestoreInstanceOrchestration` (NEW)

**Purpose**: Restore instance from backup

**Workflow:**
```rust
pub async fn restore_instance_orchestration(
    ctx: OrchestrationContext,
    input: RestoreInstanceInput,
) -> Result<RestoreInstanceOutput, String> {
    ctx.trace_info(format!("Starting restore for instance: {}", input.instance_name));
    
    // Step 1: Validate backup exists
    let backup_info = get_backup_info(input.backup_id).await?;
    
    // Step 2: Stop instance (if in-place restore)
    if !input.restore_to_new_instance {
        stop_postgres_instance(...).await?;
    }
    
    // Step 3: Perform restore
    let restore_result = restore_from_backup(...).await?;
    
    // Step 4: Start instance
    start_postgres_instance(...).await?;
    
    // Step 5: Verify restoration
    test_connection(...).await?;
    
    // Step 6: Record restore event
    record_restore_event(...).await?;
    
    ctx.trace_info("Restore completed successfully");
    
    Ok(RestoreInstanceOutput {
        restored_to: restore_result.restored_to_instance,
        backup_timestamp: backup_info.created_at,
    })
}
```

### Modified: `CreateInstanceOrchestration`

**Add backup setup:**
```rust
// After instance is running...

// NEW: Step N: Create backup CronJobs
if input.backup_enabled.unwrap_or(true) {
    ctx.trace_info("Setting up automated backups");
    
    // Daily backup CronJob
    create_backup_cronjob(
        instance_name: input.name.clone(),
        backup_type: "daily",
        schedule: "0 2 * * *",
        retention_days: input.backup_retention_days.unwrap_or(7),
    ).await?;
    
    // Weekly backup CronJob
    create_backup_cronjob(
        instance_name: input.name.clone(),
        backup_type: "weekly",
        schedule: "0 1 * * 0",
        retention_days: input.backup_retention_weeks.unwrap_or(4) * 7,
    ).await?;
    
    ctx.trace_info("Automated backups configured");
}
```

### Modified: `DeleteInstanceOrchestration`

**Add backup cleanup:**
```rust
// Before deleting instance...

// NEW: Delete backup CronJobs
ctx.trace_info("Removing backup CronJobs");
delete_backup_cronjob(instance_name, "daily").await?;
delete_backup_cronjob(instance_name, "weekly").await?;

// Optionally: Delete backup files from storage
if input.delete_backups {
    delete_all_backups_from_storage(instance_name).await?;
}
```

---

## CLI Commands

### Create with Backup Options

```bash
# Default: backups enabled
toygres create mydb --password secret123

# Disable backups
toygres create mydb --password secret123 --no-backup

# Custom retention
toygres create mydb --password secret123 \
  --backup-daily-retention 14 \
  --backup-weekly-retention 8

# Custom schedule
toygres create mydb --password secret123 \
  --backup-schedule-daily "0 3 * * *"   # 3 AM instead of 2 AM
```

### New Backup Commands

```bash
# List backups for an instance
toygres backup list mydb

# Trigger manual backup
toygres backup create mydb [--label "pre-migration"]

# View backup details
toygres backup get <backup-id>

# Restore from backup
toygres backup restore mydb --backup-id <id>

# Restore to new instance
toygres backup restore mydb --backup-id <id> --new-name mydb-restored

# Delete old backups
toygres backup cleanup mydb --older-than 30d

# Verify backup integrity
toygres backup verify <backup-id>
```

### Enhanced Get Command

```bash
toygres get mydb

# New section in output:
Backups:
  Enabled:            Yes
  Daily Schedule:     2:00 AM (UTC)
  Weekly Schedule:    Sunday 1:00 AM (UTC)
  Last Backup:        2 hours ago (2025-11-17 02:00:00)
  Last Backup Status: Success
  Last Backup Size:   245 MB
  Total Backups:      10 (7 daily, 3 weekly)
  Storage Used:       2.1 GB
  Next Backup:        In 22 hours
```

---

## Azure Blob Storage Setup

### Storage Account

**Name:** `toygresstorage` (or configurable)
**Type:** StorageV2 (general purpose v2)
**Replication:** LRS (locally redundant) or GRS (geo-redundant)
**Region:** Same as AKS cluster (westus3)

### Container

**Name:** `toygres-backups`
**Access:** Private (no public access)
**Lifecycle Management:**

```json
{
  "rules": [
    {
      "name": "MoveToArchiveAfter90Days",
      "type": "Lifecycle",
      "definition": {
        "filters": {
          "blobTypes": ["blockBlob"],
          "prefixMatch": ["*/monthly/", "*/quarterly/"]
        },
        "actions": {
          "baseBlob": {
            "tierToArchive": {
              "daysAfterModificationGreaterThan": 90
            }
          }
        }
      }
    },
    {
      "name": "DeleteAfter365Days",
      "type": "Lifecycle",
      "definition": {
        "filters": {
          "blobTypes": ["blockBlob"]
        },
        "actions": {
          "baseBlob": {
            "delete": {
              "daysAfterModificationGreaterThan": 365
            }
          }
        }
      }
    }
  ]
}
```

### Authentication

**Option 1: Managed Identity** (Recommended)
- Assign managed identity to AKS nodes
- Grant "Storage Blob Data Contributor" role
- No credentials needed in code

**Option 2: Storage Account Key**
- Store in Kubernetes Secret
- Less secure, but simpler setup

**Option 3: SAS Token**
- Time-limited access
- Can be rotated
- Good for temporary access

---

## Backup Script (Python)

**ConfigMap: `backup-scripts`**

```python
#!/usr/bin/env python3
# cleanup_old_backups.py

import os
import sys
from datetime import datetime, timedelta
from azure.storage.blob import BlobServiceClient

def cleanup_old_backups(instance_name, backup_type, keep_days=None, keep_weeks=None):
    """Clean up old backups based on retention policy"""
    
    # Connect to Azure Storage
    account_name = os.environ.get('AZURE_STORAGE_ACCOUNT', 'toygresstorage')
    connection_string = os.environ['AZURE_STORAGE_CONNECTION_STRING']
    
    blob_service = BlobServiceClient.from_connection_string(connection_string)
    container = blob_service.get_container_client('toygres-backups')
    
    # List all backups for this instance and type
    prefix = f"{instance_name}/{backup_type}/"
    blobs = container.list_blobs(name_starts_with=prefix)
    
    # Calculate cutoff date
    if keep_days:
        cutoff_date = datetime.utcnow() - timedelta(days=keep_days)
    elif keep_weeks:
        cutoff_date = datetime.utcnow() - timedelta(weeks=keep_weeks)
    else:
        return
    
    # Delete old backups
    deleted_count = 0
    deleted_size = 0
    
    for blob in blobs:
        if blob.last_modified < cutoff_date:
            print(f"Deleting old backup: {blob.name}")
            container.delete_blob(blob.name)
            deleted_count += 1
            deleted_size += blob.size
    
    print(f"Cleanup complete: Deleted {deleted_count} backups ({deleted_size} bytes)")

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--instance', required=True)
    parser.add_argument('--type', required=True)
    parser.add_argument('--keep-days', type=int)
    parser.add_argument('--keep-weeks', type=int)
    
    args = parser.parse_args()
    cleanup_old_backups(args.instance, args.type, args.keep_days, args.keep_weeks)
```

---

## Service Account & RBAC

### Kubernetes Service Account

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: toygres-backup
  namespace: toygres
  annotations:
    # For Azure Managed Identity (workload identity)
    azure.workload.identity/client-id: "<managed-identity-client-id>"
```

### RBAC Permissions

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: toygres-backup
  namespace: toygres
rules:
- apiGroups: [""]
  resources: ["pods", "services"]
  verbs: ["get", "list"]
- apiGroups: ["batch"]
  resources: ["jobs", "cronjobs"]
  verbs: ["get", "list", "create", "delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: toygres-backup
  namespace: toygres
subjects:
- kind: ServiceAccount
  name: toygres-backup
roleRef:
  kind: Role
  name: toygres-backup
  apiGroup: rbac.authorization.k8s.io
```

### Azure RBAC

```bash
# Grant backup service account access to storage
az role assignment create \
  --role "Storage Blob Data Contributor" \
  --assignee <managed-identity-principal-id> \
  --scope /subscriptions/<sub>/resourceGroups/<rg>/providers/Microsoft.Storage/storageAccounts/toygresstorage
```

---

## Monitoring & Alerts

### Backup Metrics

Track in CMS database and expose via API:

1. **Backup Success Rate**: % of successful backups in last 7 days
2. **Backup Duration**: Time taken for each backup
3. **Backup Size**: Trend over time
4. **Last Successful Backup**: Age of most recent backup
5. **Failed Backups**: Count and reasons

### Alerts

**Critical Alerts:**
- ❌ No successful backup in 48 hours
- ❌ Backup failed 3 times in a row
- ❌ Storage quota exceeded

**Warning Alerts:**
- ⚠️ Backup duration increasing (>2x baseline)
- ⚠️ Backup size increasing rapidly
- ⚠️ Storage approaching quota limit

### Dashboard Integration

Add to `toygres server stats`:

```
Backups (Last 24h):
  Successful:        48 / 50  (96%)
  Failed:            2
  Total Size:        12.4 GB
  Avg Duration:      2m 15s
  
Failed Backups:
  mydb-abc123:  Connection timeout (retry scheduled)
  testdb-xyz:   Disk full on backup pod
```

---

## Cost Estimates

### Per Instance (10GB database)

**Storage Costs (Azure Blob Hot tier):**
- Daily backups (7 days × ~3GB compressed) = 21 GB
- Weekly backups (4 weeks × ~8GB) = 32 GB
- **Total: ~50 GB per instance**
- **Cost: ~$1.20/month** (Hot tier: $0.024/GB/month)

**Compute Costs:**
- Daily backup: ~5 minutes/day = ~2.5 hours/month
- Weekly backup: ~15 minutes/week = ~1 hour/month
- **Cost: Negligible** (uses spot-like pricing for Jobs)

**Network Costs:**
- Egress to Azure Storage: Usually free (same region)
- **Cost: $0**

**Total Additional Cost per Instance: ~$1.20-2/month**

---

## Implementation Phases

### Phase 1: Basic Backups (Essential)
**Estimated: 2-3 days**

1. ⬜ Create CronJob templates (daily, weekly)
2. ⬜ Set up Azure Blob Storage container
3. ⬜ Configure managed identity / storage auth
4. ⬜ Implement `create_backup_cronjob` activity
5. ⬜ Implement `delete_backup_cronjob` activity
6. ⬜ Test manual backup creation

### Phase 2: Orchestration Integration
**Estimated: 1-2 days**

7. ⬜ Add CMS schema migration (0002_add_backups.sql)
8. ⬜ Modify `CreateInstanceOrchestration` to create CronJobs
9. ⬜ Modify `DeleteInstanceOrchestration` to clean up CronJobs
10. ⬜ Add backup fields to input types
11. ⬜ End-to-end testing

### Phase 3: Backup Management
**Estimated: 2-3 days**

12. ⬜ Implement `trigger_manual_backup` activity
13. ⬜ Implement `list_backups` activity
14. ⬜ Create `BackupInstanceOrchestration`
15. ⬜ Add CLI commands (backup list, backup create)
16. ⬜ Add backup info to `toygres get` output

### Phase 4: Restore Capability
**Estimated: 2-3 days**

17. ⬜ Implement `restore_from_backup` activity
18. ⬜ Create `RestoreInstanceOrchestration`
19. ⬜ Add CLI commands (backup restore)
20. ⬜ Test restore scenarios (in-place, new instance)

### Phase 5: Monitoring & Verification
**Estimated: 1-2 days**

21. ⬜ Implement `verify_backup` activity
22. ⬜ Add backup metrics to health checks
23. ⬜ Add backup stats to `toygres server stats`
24. ⬜ Set up backup failure alerts
25. ⬜ Create backup verification CronJob (weekly)

### Phase 6: Advanced Features (Optional)
**Estimated: 2-3 days**

26. ⬜ Implement WAL archiving for PITR
27. ⬜ Add backup retention policy configurator
28. ⬜ Implement backup encryption
29. ⬜ Add backup compression options
30. ⬜ Create backup dashboard

---

## Testing Strategy

### Manual Testing

```bash
# 1. Create instance with backups
./toygres create testbackup --password test123

# 2. Verify CronJobs created
kubectl get cronjobs -n toygres | grep testbackup

# 3. Trigger manual backup
./toygres backup create testbackup

# 4. List backups
./toygres backup list testbackup

# 5. Insert test data
psql -h testbackup... -c "CREATE TABLE test(id INT); INSERT INTO test VALUES (1,2,3);"

# 6. Restore to new instance
./toygres backup restore testbackup --backup-id <id> --new-name testbackup-restored

# 7. Verify data restored
psql -h testbackup-restored... -c "SELECT * FROM test;"

# 8. Delete instance (choose to keep/delete backups)
./toygres delete testbackup --keep-backups
```

### Automated Tests

```rust
#[tokio::test]
async fn test_create_backup_cronjob() {
    // Test CronJob creation
}

#[tokio::test]
async fn test_manual_backup_trigger() {
    // Test manual backup execution
}

#[tokio::test]
async fn test_backup_restore() {
    // Test full restore workflow
}

#[tokio::test]
async fn test_backup_retention() {
    // Test cleanup of old backups
}
```

---

## Security Considerations

### 1. Backup Encryption

**At-rest:** Azure Blob Storage encrypts by default (256-bit AES)

**In-transit:** Use HTTPS for all Azure SDK calls

**Optional:** Client-side encryption before upload:
```bash
# Encrypt before upload
gpg --symmetric --cipher-algo AES256 backup.sql.gz
az storage blob upload ... --file backup.sql.gz.gpg
```

### 2. Access Control

- Use Azure RBAC for storage access
- Kubernetes RBAC for CronJob management
- Separate backup service account (principle of least privilege)
- Audit logging for all backup/restore operations

### 3. Password Handling

**Don't store in backup metadata:**
- Backups contain database, but not the password
- Password stays in Kubernetes Secret
- For restore: Use same secret or provide new password

---

## Failure Scenarios & Recovery

### Scenario 1: Backup Job Fails

**Detection:** CronJob reports failure, CMS shows failed status

**Recovery:**
1. Check logs: `kubectl logs job/testbackup-daily-backup-<id>`
2. Investigate error (network, permissions, disk full)
3. Retry: Next scheduled run or manual trigger
4. Alert operators if 3 consecutive failures

### Scenario 2: Azure Storage Unavailable

**Detection:** Backup pod can't connect to Azure

**Recovery:**
1. Backups queue in pod (if disk space available)
2. Retry with exponential backoff
3. Alert operators
4. Manual intervention if prolonged

### Scenario 3: Backup Corrupted

**Detection:** Verify backup job fails

**Recovery:**
1. Mark backup as invalid in CMS
2. Trigger immediate replacement backup
3. Investigate corruption cause
4. Use previous day's backup if needed

### Scenario 4: Restore Fails

**Detection:** Restore orchestration errors

**Recovery:**
1. Verify backup file exists and is downloadable
2. Check target instance has enough disk space
3. Verify PostgreSQL version compatibility
4. Try different backup if available
5. Escalate to manual intervention

---

## Point-in-Time Recovery (PITR) - Optional Advanced Feature

### Overview

For critical databases, enable PITR to recover to any specific timestamp.

### Requirements

1. **Continuous WAL archiving** to Azure Blob Storage
2. **Base backup** (weekly pg_basebackup)
3. **WAL files** archived continuously

### Configuration

**postgresql.conf:**
```ini
wal_level = replica
archive_mode = on
archive_command = 'az storage blob upload --account-name toygresstorage --container-name toygres-backups --name %p --file %p'
```

### Restore Process

```bash
# Restore to specific point in time
toygres backup restore mydb \
  --pitr "2025-11-17 14:30:00" \
  --base-backup <weekly-backup-id>
```

**Steps:**
1. Download base backup
2. Extract to data directory
3. Download all WAL files since base backup
4. Configure recovery.conf with target time
5. Start PostgreSQL (replays WAL until target)
6. Promote to primary

---

## Performance Optimization

### 1. Parallel Compression

Use `pigz` instead of `gzip` for faster compression:
```bash
pg_dump ... | pigz -p 4 > backup.sql.gz
```

### 2. Incremental Backups

Use PostgreSQL 15+ incremental backup feature:
```bash
pg_basebackup --incremental=<manifest>
```

### 3. Backup Window

Schedule during low-usage hours (2-4 AM typically)

### 4. Network Optimization

- Use Azure private endpoints
- Co-locate storage in same region
- Use premium storage for faster writes

---

## Migration Strategy

### Step 1: Infrastructure Setup (Week 1)

1. Create Azure Storage Account
2. Create container with lifecycle policy
3. Configure managed identity
4. Test manual backup/restore

### Step 2: Activity Implementation (Week 2)

1. Implement backup CronJob creation
2. Implement manual backup trigger
3. Test in dev environment

### Step 3: Orchestration Integration (Week 3)

1. Modify create/delete orchestrations
2. Add CMS schema migration
3. Integration testing

### Step 4: CLI & Monitoring (Week 4)

1. Add backup CLI commands
2. Add backup metrics to stats
3. Set up alerting
4. Documentation

### Step 5: Rollout (Week 5)

1. Enable for new instances (flag-based)
2. Monitor first week of backups
3. Backfill existing instances
4. Make default for all instances

---

## Open Questions

1. **Should backups be encrypted client-side?**
   - Azure encrypts at-rest, but client-side adds extra layer
   - Adds complexity (key management)
   - Recommendation: Start with Azure encryption, add client-side if needed

2. **What's the default retention policy?**
   - Daily: 7 days (reasonable for most use cases)
   - Weekly: 4 weeks (1 month)
   - Monthly: 3 months (compliance)
   - Recommendation: Make configurable per instance

3. **Should we support backup to multiple destinations?**
   - Azure Blob + S3?
   - Different Azure regions?
   - Recommendation: Single location initially, add multi-region later

4. **How to handle backup of very large databases (>100GB)?**
   - pg_dump becomes slow
   - Consider pg_basebackup only
   - Or parallel pg_dump
   - Recommendation: Automatic selection based on size

5. **Should restore be in-place or always to new instance?**
   - In-place: Faster, but destructive
   - New instance: Safer, allows validation
   - Recommendation: Default to new instance, allow in-place with --force

---

## Success Criteria

### Functional Requirements

- ✅ Automated daily backups for all instances
- ✅ Automated weekly backups for all instances
- ✅ Manual backup triggering on-demand
- ✅ Backup listing and details viewing
- ✅ Successful restore from backup
- ✅ Automatic cleanup of old backups
- ✅ Backup failure detection and alerting

### Performance Requirements

- ✅ Daily backup completes in < 15 minutes (for 10GB database)
- ✅ Weekly backup completes in < 30 minutes
- ✅ Restore completes in < 30 minutes
- ✅ Backup operations don't impact instance performance (< 5% CPU overhead)

### Reliability Requirements

- ✅ Backup success rate > 99%
- ✅ Backups are restorable and verified
- ✅ No backup data loss in storage
- ✅ Automatic retry on transient failures

---

## Cost Summary

### Per Instance Per Month

**10GB Database:**
- Storage: ~$1.20 (50GB in Hot tier)
- Compute: < $0.50 (backup job execution)
- Network: $0 (same region)
- **Total: ~$1.70/month**

**100GB Database:**
- Storage: ~$12 (500GB in Hot tier)
- Compute: < $2 (longer backup jobs)
- Network: $0
- **Total: ~$14/month**

### Optimization Options

- Use Cool tier for old backups (-50% storage cost)
- Use Archive tier for quarterly backups (-80% storage cost)
- Compress backups aggressively (-30-50% size)

---

## References

- [PostgreSQL Backup Documentation](https://www.postgresql.org/docs/current/backup.html)
- [pg_dump](https://www.postgresql.org/docs/current/app-pgdump.html)
- [pg_basebackup](https://www.postgresql.org/docs/current/app-pgbasebackup.html)
- [Azure Blob Storage](https://learn.microsoft.com/en-us/azure/storage/blobs/)
- [Kubernetes CronJobs](https://kubernetes.io/docs/concepts/workloads/controllers/cron-jobs/)

---

## Next Steps

1. **Review this plan** and decide on initial scope
2. **Set up Azure Storage Account** and test manual upload
3. **Create CronJob templates** for daily/weekly backups
4. **Implement Phase 1 activities**
5. **Test with single instance** before rolling out
6. **Decide:** Start with backups or replication first?


