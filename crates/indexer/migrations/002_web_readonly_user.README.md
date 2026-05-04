# Migration 002 — `yog_web` read-only role

## Purpose

The Next.js dashboard (`/web`) reads from the same TimescaleDB
instance that the indexer writes to. Following the principle of least
privilege, we provision a dedicated role for the web process with
`SELECT`-only rights.

## Apply the migration

The migration is idempotent and safe to run more than once:

```bash
psql "$DATABASE_URL" -f crates/indexer/migrations/002_web_readonly_user.sql
```

## Set the password (out-of-band)

The migration creates the role with `NOLOGIN` and no password.
After applying it, set a strong password and enable login:

```bash
psql "$DATABASE_URL" -c \
  "ALTER ROLE yog_web WITH LOGIN PASSWORD '<provisioned-secret>';"
```

Store the secret in your password manager and inject it into the web
app via the `DATABASE_URL` env var (see `web/.env.example`):

```
DATABASE_URL=postgresql://yog_web:<provisioned-secret>@<host>:5432/yog_sothoth?sslmode=require
```

## Verify

Connect as `yog_web` and confirm that reads work and writes do not:

```bash
psql "postgresql://yog_web:<password>@<host>:5432/yog_sothoth"

-- Should succeed:
SELECT count(*) FROM pools;

-- Should fail with `permission denied`:
INSERT INTO pools (pool_address, protocol) VALUES ('test', 'damm_v2');
```

## Future tables

Tables created after this migration runs are automatically granted
`SELECT` to `yog_web` via `ALTER DEFAULT PRIVILEGES`. No re-run is
required when the schema evolves — but only as long as the new tables
are owned by the same role that ran this migration. If a different
role becomes the owner of a future table, adjust the
`ALTER DEFAULT PRIVILEGES ... FOR ROLE` clause accordingly.