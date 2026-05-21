# Migrations — yog-persistence

Applied by the `yog-migrate` binary at startup of the Docker compose
stack, and manually during development via:

```sh
cargo run --bin yog-migrate -p yog-persistence
```

The connection string passed to `yog-migrate` must use the `yog_migrate`
role — the runtime roles (`yog_indexer`, `yog_api`, `yog_context`)
intentionally cannot CREATE or ALTER tables.

## Forward-only

Migrations are forward-only. **A migration committed to git never
changes.** No `.down.sql` files; no edits to past migrations. If a
released migration introduced a problem, fix it forward by writing a
new migration that corrects the state.

This is the right discipline for production safety:

- Reversing schema changes generally loses data anyway (a dropped
  column cannot be reconstructed).
- The hash of every applied migration is tracked in `_sqlx_migrations`;
  modifying a file would break every database that already applied it.
- "How do I roll back?" is answered by **backups** (pg_dump / Scaleway
  snapshots), not by reverse SQL. Before applying a fragile migration
  locally, run `pg_dump` first.

## Convention for new migrations

Each migration that creates a table emits its GRANT statements at the
end of the relevant section, in the same file. SELECT is covered by
the default privileges set in `setup_roles.sql`; everything else is
explicit per migration.

```sql
-- 002_new_event_table.sql

CREATE TABLE new_event_table (
    ...
);

CREATE INDEX ... ;

-- Grants — defaults cover SELECT for the three runtime roles; INSERT
-- / UPDATE goes here.
GRANT INSERT, UPDATE ON new_event_table TO yog_indexer;
```

The static structural grants (schema ownership, default privileges,
sequences default) live in `setup_roles.sql` at the parent directory —
that file is the provisioning one-shot, applied by hand with the
admin role when a new database is created.

## Local development workflow

When you add a new migration:

1. Create `00X_xxx.sql` here.
2. Apply it locally:
   ```sh
   cargo run --bin yog-migrate -p yog-persistence
   ```
   (Reads `DATABASE_URL_MIGRATE` from `.env`; must be the yog_migrate
   role.)
3. Regenerate the `.sqlx/` offline metadata if you used the `query!`
   macros against the new schema:
   ```sh
   cd crates/persistence
   cargo sqlx prepare
   ```
4. Commit the new migration AND the updated `.sqlx/` snapshot.

## Bootstrapping a fresh database

The first-time setup, against an empty database:

```sh
# 1. As the superuser, declare the four roles + structural privileges.
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f crates/persistence/setup_roles.sql

# 2. Apply all migrations as yog_migrate.
cargo run --bin yog-migrate -p yog-persistence
```

After step 2, the runtime services (indexer / api / context) can
connect with their respective roles.