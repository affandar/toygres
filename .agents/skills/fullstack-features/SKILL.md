---
name: fullstack-features
description: Building full-stack features in Toygres from UI to database. Use when adding new features, API endpoints, React components, or implementing end-to-end functionality.
---

# Full-Stack Feature Development

## Overview
Adding a complete feature from UI to database in Toygres.

## Checklist

1. [ ] Database migration (if new data)
2. [ ] Activity (if durable operation needed)
3. [ ] Orchestration (if multi-step workflow)
4. [ ] API endpoint in `toygres-server/src/api.rs`
5. [ ] TypeScript types in `toygres-ui/src/lib/types.ts`
6. [ ] API function in `toygres-ui/src/lib/api.ts`
7. [ ] UI component update
8. [ ] Build both: `cargo build --workspace && cd toygres-ui && npm run build`
9. [ ] Deploy: `./deploy/deploy-to-aks.sh --https`

## Image Type Notes

**`pg_durable` is a built-in special mode** — it is NOT a user-uploadable runtime image.

When adding runtime image support:
- Runtime images from ACR are always deployed in **stock** mode
- Only the built-in `pg_durable` uses special env vars and pg_hba.conf configuration
- Force `image_type = 'stock'` when a `runtime_image_id` is provided

## API Endpoint Pattern

```rust
// In toygres-server/src/api.rs

async fn my_endpoint(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Your logic here
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Operation completed"
    })))
}

// Add route in create_router():
.route("/api/instances/:name/my-action", post(my_endpoint))
```

## Frontend API Function

```typescript
// In toygres-ui/src/lib/api.ts
async myAction(name: string): Promise<{ success: boolean; message: string }> {
  const response = await this.fetch(`/api/instances/${encodeURIComponent(name)}/my-action`, {
    method: 'POST',
  });
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.error || 'Action failed');
  }
  return response.json();
}
```

## React Mutation Pattern

```typescript
const myMutation = useMutation({
  mutationFn: (name: string) => api.myAction(name),
  onSuccess: (data) => {
    queryClient.invalidateQueries({ queryKey: ['instance', name] });
    showToast('success', data.message);
  },
  onError: (error: Error) => {
    showToast('error', `Failed: ${error.message}`);
  },
});

// In JSX:
<Button onClick={() => myMutation.mutate(name)} disabled={myMutation.isPending}>
  {myMutation.isPending ? 'Working...' : 'Do Action'}
</Button>
```

## Simple vs Durable Operations

**Simple/Atomic** (direct K8s or DB call):
- UI → API → K8s/Database
- Example: Stop instance (scale replicas to 0)

**Durable** (multi-step, needs retry/recovery):
- UI → API → Start Orchestration → Activities
- Example: Create instance (deploy + wait + test connection)

## Catalog Entity Pattern

For adding a new "catalog" of entities (like runtime images):

1. **Migration**: Create table with id, name, metadata columns
2. **API**: Add list + register endpoints
3. **Types**: Add TypeScript interface
4. **API Client**: Add fetch functions
5. **UI**: Create list/form component
6. **Sidebar**: Add navigation entry

See `database-changes` skill for detailed SQL patterns.

