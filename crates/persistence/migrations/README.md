# Migrations — yog-persistence

These migrations are applied by the `yog-migrate` binary at startup
of the Docker compose stack, and manually during development via
`cargo run --bin yog-migrate -p yog-persistence`.

The connection string passed to `yog-migrate` must use the
`yog_migrate` role — the runtime roles (`yog_indexer`, `yog_api`,
`yog_context`) intentionally cannot CREATE or ALTER tables.

## Convention for new migrations

Each migration that creates a new table MUST emit the GRANT
statements for the runtime roles, at the bottom of the file:

```sql
-- 005_new_event_table.sql

CREATE TABLE new_event_table (
    ...
);

-- Grants — the default privileges only cover SELECT for the three
-- runtime roles. Explicit INSERT/UPDATE goes here.
GRANT INSERT, UPDATE ON new_event_table TO yog_indexer;
-- yog_api / yog_context already get SELECT through default privs.
```

This keeps every migration self-contained: the table and its
permission contract are version-controlled in the same file.

The static grants for tables that exist before any migration runs
(or that need a special grant pattern) live in `setup_roles.sql` at
the parent directory — that file is the provisioning one-shot,
applied by hand with the admin role when a new database is created.

## Local development workflow

When you add a new migration:

1. Create `00X_xxx.sql` here.
2. Apply it locally:
   ```sh
   cargo run --bin yog-migrate -p yog-persistence
   ```
   (Reads `DATABASE_URL` from `.env`; must be the yog_migrate role.)
3. Regenerate the `.sqlx/` offline metadata if you used the
   `query!` macros against the new schema:
   ```sh
   cd crates/persistence
   cargo sqlx prepare
   ```
4. Commit the new migration AND the updated `.sqlx/` snapshot.