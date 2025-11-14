# CMS Activity Idempotency Patterns

## Why Idempotency Matters

Duroxide orchestrations can be **replayed** after:
- Worker crashes
- Orchestration failures with retries
- Debugging/replay scenarios

**Without idempotency:**
- Duplicate database records
- Invalid state transitions
- Data corruption

**With idempotency:**
- ✅ Same operation executed multiple times → same result
- ✅ Safe to replay orchestrations
- ✅ Consistent state across retries

---

## Pattern 1: UPSERT for CREATE Operations

**Use Case:** Creating a CMS record that might already exist

**Problem:**
```sql
-- NOT idempotent - fails on second execution
INSERT INTO instances (k8s_name, state, ...) VALUES ('mydb-abc123', 'creating', ...);
-- Error: duplicate key value violates unique constraint
```

**Solution:**
```sql
-- IDEMPOTENT - returns existing record on conflict
INSERT INTO instances (k8s_name, state, ...) 
VALUES ('mydb-abc123', 'creating', ...)
ON CONFLICT (k8s_name) DO UPDATE 
SET create_orchestration_id = EXCLUDED.create_orchestration_id
RETURNING id;
```

**Result:** First call creates record, subsequent calls return existing ID.

---

## Pattern 2: Conditional Updates for STATE Changes

**Use Case:** Updating instance state

**Problem:**
```sql
-- NOT idempotent - overwrites state even if already correct
UPDATE instances SET state = 'running', updated_at = NOW() WHERE k8s_name = 'mydb-abc123';
-- On replay: updated_at changes even though state was already 'running'
```

**Solution:**
```sql
-- IDEMPOTENT - only updates if state is different
UPDATE instances 
SET state = 'running', 
    ip_connection_string = $1,
    updated_at = CASE WHEN state != 'running' THEN NOW() ELSE updated_at END
WHERE k8s_name = 'mydb-abc123' 
  AND state != 'running';
```

Or check in application code:

```rust
// Fetch current state
let current_state = query!("SELECT state FROM instances WHERE k8s_name = $1", k8s_name)
    .fetch_one(&pool).await?.state;

// Only update if different
if current_state != new_state {
    query!("UPDATE instances SET state = $1 WHERE k8s_name = $2", new_state, k8s_name)
        .execute(&pool).await?;
    
    // Log event
    query!("INSERT INTO instance_events (instance_id, event_type, old_state, new_state) ...")
        .execute(&pool).await?;
} else {
    ctx.trace_info("State already set, skipping update (replay)");
}
```

**Result:** No-op on replay if state already correct.

---

## Pattern 3: Unique Constraints for Event Logs

**Use Case:** Recording health check results or events

**Problem:**
```sql
-- NOT idempotent - creates duplicate records on replay
INSERT INTO instance_health_checks (instance_id, status, checked_at) 
VALUES (..., 'healthy', NOW());
-- On replay: multiple identical health checks with different NOW() values
```

**Solution:**
```sql
-- IDEMPOTENT - use deterministic timestamp from orchestration context
INSERT INTO instance_health_checks (instance_id, status, checked_at) 
VALUES (..., 'healthy', $orchestration_time)  -- $orchestration_time from ctx.utcnow_ms()
ON CONFLICT (instance_id, checked_at) DO NOTHING;
```

With unique constraint:
```sql
ALTER TABLE instance_health_checks 
ADD CONSTRAINT unique_health_check_time UNIQUE (instance_id, checked_at);
```

**Result:** Duplicate inserts are silently ignored.

---

## Pattern 4: State Machine Transitions

**Use Case:** Ensuring valid state transitions

**Problem:**
```sql
-- Can transition from any state to any state
UPDATE instances SET state = 'running' WHERE k8s_name = ...;
-- Replay might transition from 'running' to 'creating' (invalid!)
```

**Solution:**
```sql
-- Only allow valid transitions
UPDATE instances 
SET state = 'running' 
WHERE k8s_name = $1 
  AND state = 'creating';  -- Only from 'creating' state

-- Check rows affected
```

In Rust:
```rust
let rows_affected = sqlx::query!(
    "UPDATE instances SET state = 'running' WHERE k8s_name = $1 AND state = 'creating'",
    k8s_name
)
.execute(&pool)
.await?
.rows_affected();

if rows_affected == 0 {
    // Either already in 'running' state (replay) or invalid transition
    let current = sqlx::query!("SELECT state FROM instances WHERE k8s_name = $1", k8s_name)
        .fetch_one(&pool).await?;
    
    if current.state == "running" {
        ctx.trace_info("Already in 'running' state (replay), continuing");
    } else {
        return Err(format!("Invalid state transition: {} → running", current.state));
    }
}
```

**Result:** Prevents invalid state transitions, handles replays gracefully.

---

## Pattern 5: Deterministic Timestamps

**Use Case:** Recording events with timestamps

**Problem:**
```rust
// NOT deterministic - NOW() returns different value on each replay
let timestamp = chrono::Utc::now();  // ❌ Changes on replay
```

**Solution:**
```rust
// IDEMPOTENT - use orchestration context time (replay-safe)
let timestamp_ms = ctx.utcnow_ms().await?;
let timestamp = chrono::DateTime::from_timestamp_millis(timestamp_ms as i64)
    .ok_or("Invalid timestamp")?;

// Use this timestamp in database operations
query!("INSERT INTO events (..., created_at) VALUES (..., $1)", timestamp)
    .execute(&pool).await?;
```

**Result:** Same timestamp on replay, enabling idempotent inserts with unique constraints.

---

## Pattern 6: COALESCE for Partial Updates

**Use Case:** Updating only some fields without overwriting others

**Problem:**
```sql
-- Overwrites fields with NULL on replay if not provided
UPDATE instances 
SET ip_connection_string = $1, dns_connection_string = $2
WHERE k8s_name = $3;
-- If $1 is NULL, wipes out existing connection string!
```

**Solution:**
```sql
-- IDEMPOTENT - only updates non-NULL values
UPDATE instances 
SET ip_connection_string = COALESCE($1, ip_connection_string),
    dns_connection_string = COALESCE($2, dns_connection_string),
    updated_at = CASE 
        WHEN $1 IS NOT NULL OR $2 IS NOT NULL THEN NOW() 
        ELSE updated_at 
    END
WHERE k8s_name = $3;
```

**Result:** Preserves existing values if new value is NULL.

---

## Pattern 7: Soft Deletes

**Use Case:** Marking instances as deleted without losing history

**Problem:**
```sql
-- Hard delete loses all history
DELETE FROM instances WHERE k8s_name = ...;
-- Can't audit, can't track what was deleted
```

**Solution:**
```sql
-- IDEMPOTENT soft delete
UPDATE instances 
SET state = 'deleted', 
    deleted_at = COALESCE(deleted_at, NOW())  -- Keep original deletion time
WHERE k8s_name = $1 
  AND state != 'deleted';

-- Query only active instances
SELECT * FROM instances WHERE state != 'deleted';
```

**Result:** Multiple delete calls are no-ops after first one.

---

## Summary: Idempotency Checklist

For each CMS activity, ensure:

✅ **Creates** - Use `ON CONFLICT DO UPDATE` (UPSERT)  
✅ **Updates** - Check current value, only update if different  
✅ **Inserts** - Use unique constraints + `ON CONFLICT DO NOTHING`  
✅ **Deletes** - Use soft deletes with `deleted_at` timestamp  
✅ **Timestamps** - Use `ctx.utcnow_ms()`, not `NOW()` or `Utc::now()`  
✅ **State transitions** - Validate source state before transition  
✅ **Partial updates** - Use `COALESCE()` to preserve existing values  
✅ **Event logs** - Deduplicate using unique constraints on meaningful fields  

---

## Testing Idempotency

### Unit Test Pattern

```rust
#[tokio::test]
async fn test_update_instance_state_idempotent() {
    let pool = setup_test_db().await;
    
    let input = UpdateInstanceStateInput {
        k8s_name: "test-abc123".to_string(),
        state: "running".to_string(),
        // ...
    };
    
    // First call - should update
    let result1 = update_instance_state_activity(ctx.clone(), input.clone()).await.unwrap();
    assert!(result1.success);
    
    // Second call (replay) - should be no-op
    let result2 = update_instance_state_activity(ctx.clone(), input.clone()).await.unwrap();
    assert!(result2.success);
    
    // Verify state is correct and only one event logged
    let state = get_instance_state(&pool, "test-abc123").await;
    assert_eq!(state, "running");
    
    let event_count = count_state_change_events(&pool, "test-abc123").await;
    assert_eq!(event_count, 1);  // Only one event, not two!
}
```

---

## Common Pitfalls to Avoid

❌ **Using NOW() or Utc::now()** - Not replay-safe  
✅ **Use ctx.utcnow_ms()** - Recorded in orchestration history

❌ **Unconditional UPDATEs** - Changes data even if already correct  
✅ **Check before update** - Only update if different

❌ **INSERT without ON CONFLICT** - Fails on replay  
✅ **Use UPSERT or DO NOTHING** - Handles duplicates

❌ **No unique constraints** - Can't enforce idempotency  
✅ **Add unique constraints** - Database enforces idempotency

❌ **Hard deletes** - Lose audit trail  
✅ **Soft deletes** - Maintain history

---

## Implementation Notes

When implementing CMS activities:

1. **Always fetch current state first** before deciding to update
2. **Use deterministic timestamps** from orchestration context
3. **Add unique constraints** for any INSERT operations
4. **Log decisions** - trace whether operation was executed or skipped
5. **Test with replays** - call activity twice with same input, verify idempotency

This ensures CMS stays consistent even with Duroxide orchestration replays! ✅

