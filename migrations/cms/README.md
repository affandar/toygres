# Toygres CMS Migrations

This directory stores schema migrations for the `toygres_cms` schema. Migrations
follow the same numbering pattern as the `duroxide-pg` project:

```
0001_initial_schema.sql
0002_add_column.sql
0003_fix_index.sql
```

Each file contains idempotent SQL statements that can run safely multiple
times. Versions are tracked in `toygres_cms._toygres_migrations` and applied in
numeric order by the helper scripts:

- `scripts/db-init.sh` – applies the initial schema (version 1) and ensures the
  Duroxide + CMS schemas exist.
- `scripts/db-migrate.sh` – applies any migrations numbered `0002` and above.

When adding a new migration:

1. Copy the next sequential filename.
2. Keep statements idempotent using `IF NOT EXISTS`.
3. Test locally by running `./scripts/db-migrate.sh`.
4. Commit both the migration file and any code changes that depend on it.

