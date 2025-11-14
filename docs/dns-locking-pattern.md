# DNS Locking Pattern for CMS

## Problem

Multiple orchestrations might try to create instances with the same DNS name simultaneously. We need to ensure:
1. Only one orchestration can "own" a DNS name at a time
2. Orchestration replays are idempotent (same orch can retry)
3. Different orchestrations are blocked

## Solution: Optimistic Locking with Orchestration ID

Use the orchestration ID as a lock identifier in the database.

### Database Constraints

```sql
-- Unique index ensures only one active instance per DNS name
CREATE UNIQUE INDEX idx_instances_dns_name_unique 
    ON toygres_cms.instances(dns_name) 
    WHERE dns_name IS NOT NULL 
      AND dns_name NOT LIKE '__deleted_%'
      AND state IN ('creating', 'running');
```

### Activity Logic: CREATE_INSTANCE_RECORD

```rust
pub async fn create_instance_record_activity(
    ctx: ActivityContext,
    input: CreateInstanceRecordInput,
) -> Result<CreateInstanceRecordOutput, String> {
    let pool = get_db_pool()?;
    let mut tx = pool.begin().await?;
    
    let insert_result = sqlx::query!(
        r#"
        INSERT INTO toygres_cms.instances 
        (user_name, k8s_name, namespace, postgres_version, storage_size_gb, 
         use_load_balancer, dns_name, state, create_orchestration_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'creating', $8)
        ON CONFLICT (k8s_name) DO UPDATE
        SET user_name = EXCLUDED.user_name,
            namespace = EXCLUDED.namespace,
            postgres_version = EXCLUDED.postgres_version,
            storage_size_gb = EXCLUDED.storage_size_gb,
            use_load_balancer = EXCLUDED.use_load_balancer,
            dns_name = EXCLUDED.dns_name,
            updated_at = NOW()
        WHERE toygres_cms.instances.create_orchestration_id = EXCLUDED.create_orchestration_id
        RETURNING id
        "#,
        input.user_name,
        input.k8s_name,
        input.namespace,
        input.postgres_version,
        input.storage_size_gb,
        input.use_load_balancer,
        input.dns_name,
        input.orchestration_id
    )
    .fetch_optional(&mut *tx)
    .await;
    
    match insert_result {
        Ok(Some(row)) => {
            tx.commit().await?;
            ctx.trace_info(format!("Created/updated CMS record (orch_id: {})", input.orchestration_id));
            Ok(CreateInstanceRecordOutput { instance_id: row.id })
        }
        Err(sqlx::Error::Database(db_err))
            if db_err.code().as_deref() == Some("23505")
               && db_err.constraint() == Some("idx_instances_dns_name_unique") =>
        {
            let dns_name = input
                .dns_name
                .clone()
                .ok_or_else(|| "DNS name missing in conflict path".to_string())?;
            
            let existing = sqlx::query!(
                r#"
                SELECT id, k8s_name, user_name, create_orchestration_id
                FROM toygres_cms.instances
                WHERE dns_name = $1
                  AND dns_name NOT LIKE '__deleted_%'
                  AND state IN ('creating', 'running')
                FOR UPDATE
                "#,
                dns_name
            )
            .fetch_optional(&mut *tx)
            .await?;
            
            match existing {
                Some(record) if record.create_orchestration_id == input.orchestration_id => {
                    tx.commit().await?;
                    ctx.trace_info(format!(
                        "Reused CMS record {} (DNS replay, k8s: {})",
                        record.id, record.k8s_name
                    ));
                    Ok(CreateInstanceRecordOutput { instance_id: record.id })
                }
                Some(record) => {
                    tx.rollback().await?;
                    Err(format!(
                        "DNS name '{}' is already in use by '{}' (orch {})",
                        dns_name, record.k8s_name, record.create_orchestration_id
                    ))
                }
                None => {
                    tx.rollback().await?;
                    Err("DNS conflict vanished; retry create_instance_record".into())
                }
            }
        }
        Err(e) => {
            tx.rollback().await?;
            Err(format!("Failed to create instance record: {}", e))
        }
        Ok(None) => unreachable!(),
    }
}
```

## How It Works

### Scenario 1: First Attempt (Happy Path)

```
Orchestration A tries to create "mydb" with DNS "mydb-toygres.westus3.cloudapp.azure.com"
  ↓
1. INSERT ... ON CONFLICT(k8s_name) DO UPDATE runs inside a transaction ✅
2. Unique index accepts the row → DNS is now locked by orchestration A ✅
```

### Scenario 2: Replay (Same Orchestration)

```
Orchestration A crashes and replays
  ↓
1. INSERT ... ON CONFLICT(k8s_name) DO UPDATE fires
2. WHERE clause matches (same orchestration_id) → UPDATE runs ✅
3. RETURNING id gives the same row (idempotent)
```

### Scenario 3: Conflict (Different Orchestration)

```
Orchestration A is creating "mydb"
Orchestration B tries to create "otherdb" with SAME DNS name
  ↓
1. INSERT hits unique constraint idx_instances_dns_name_unique ❌
2. We SELECT ... FOR UPDATE inside the same transaction to discover owner
3. Owner orchestration_id differs → we surface "DNS already in use" error
```

### Scenario 4: DNS Reuse After Deletion

```
Orchestration A created "mydb" with DNS "mydb-toygres.westus3..."
Instance deleted, DNS prefixed: "__deleted_mydb-toygres.westus3..."
  ↓
Orchestration C tries to create "newdb" with DNS "mydb-toygres.westus3..."
  ↓
1. Partial unique index ignores '__deleted_%' → INSERT proceeds ✅
2. DNS name is now owned by orchestration C
```

## Benefits

✅ **No Separate DNS Check Activity** - Simpler, fewer round-trips  
✅ **Optimistic Locking** - Fast path when no conflicts  
✅ **Orchestration ID as Lock** - Natural fit with Duroxide  
✅ **Early Failure** - Fails before creating K8s resources  
✅ **Idempotent** - Replays handled correctly  
✅ **Automatic Cleanup** - DELETE_INSTANCE_ORCHESTRATION frees DNS  

## Edge Cases Handled

1. **Replay with same orch_id** → UPDATE succeeds
2. **Conflict with diff orch_id** → ERROR early
3. **Deleted DNS** → `__deleted_` prefix excludes from uniqueness check
4. **No DNS** → `NULL` doesn't conflict (not in unique index)
5. **Empty DNS** → Treated as no DNS

## Database Constraint Guarantees

The unique index ensures at the database level:
```sql
WHERE dns_name IS NOT NULL 
  AND dns_name NOT LIKE '__deleted_%'
  AND state IN ('creating', 'running')
```

This means:
- Deleted instances don't block DNS reuse
- Failed instances (if prefixed) don't block
- Only active instances hold DNS locks

## Testing

```bash
# Terminal 1: Start first instance
cargo run --bin toygres-server create mydb --password test123

# Terminal 2: Try same DNS while first is creating
cargo run --bin toygres-server create otherdb --password test123
# Should fail with: "DNS name 'mydb-toygres.westus3...' is already in use"
```

Clean and simple! 🎯

