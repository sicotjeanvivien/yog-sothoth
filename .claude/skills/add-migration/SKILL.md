---
name: add-migration
description: Create a new forward-only SQL migration for Yog-Sothoth (TimescaleDB / yog-persistence). Use when the user wants to add or change schema — a new event table, index, column, or VIEW branch. Encodes the forward-only discipline, the TimescaleDB hypertable boilerplate, the per-role GRANT model, and the SQLx-cache regeneration step.
---

# Add a SQL migration

Authoritative refs: `crates/persistence/migrations/README.md` and `CLAUDE.md` →
*Database privilege model*. This skill is the operational checklist with the real patterns.

Migrations live in `crates/persistence/migrations/`, are applied by the **`yog-migrate`**
binary under the **`yog_migrate`** role, and are **forward-only**.

## Iron rules (a PR that breaks these won't merge)

- **Forward-only. A migration committed to git never changes.** No `.down.sql`, no edits to
  past files. Their hashes are tracked in `_sqlx_migrations`; editing one breaks every DB
  that already applied it. Fix mistakes by writing a *new* migration that corrects state.
- **Rollback = restore from backup** (pg_dump / Scaleway snapshot), not reverse SQL. Before
  a fragile migration, `pg_dump` first.
- **Least privilege.** Runtime roles (`yog_indexer`, `yog_api`, `yog_context`) cannot
  CREATE/ALTER — only `yog_migrate` does DDL. Each new table must emit its own GRANTs in the
  same file (see below).

## Before you start — clarify scope

- What changes: **new table**, **new index/column on an existing table**, or **VIEW change**?
- If a table: which **runtime roles** need write access? (indexer writes events; context
  writes `token_metadata`/`token_prices`; api is read-only everywhere.)
- Does it feed a cross-protocol VIEW (`swap_events`, `liquidity_events`,
  `claim_position_fee_events`, `claim_reward_events`)?

## Step 1 — Create the file

Name it `NNN_descriptive_name.sql`, where `NNN` is the **next integer** after the highest
existing file (check `crates/persistence/migrations/` — `ls`; do not reuse or renumber).
Open with a header comment block matching the house style (ring/voie annotations explaining
the *why*), as in `008_update_pool_fees_events.sql`.

## Step 2 — Table DDL (TimescaleDB pattern)

Event tables are TimescaleDB hypertables partitioned on `timestamp`. Copy the full shape
from a recent migration (`008_update_pool_fees_events.sql` is a clean template):

```sql
CREATE TABLE meteora_<product>_<event_kind>_events (
    id            BIGSERIAL,
    pool_address  TEXT        NOT NULL,
    signature     TEXT        NOT NULL,
    -- … protocol-relevant columns only — NO NULL columns for incompatible
    --    fields, NO JSONB catch-all. Lossless u128 → NUMERIC(39,0).
    timestamp     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, timestamp)            -- timestamp must be in the PK (hypertable)
);

SELECT create_hypertable('meteora_<product>_<event_kind>_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');

CREATE INDEX ON meteora_<product>_<event_kind>_events (pool_address, timestamp DESC);
-- Idempotency guard against re-ingesting the same signature:
CREATE UNIQUE INDEX ON meteora_<product>_<event_kind>_events (signature, timestamp);

ALTER TABLE meteora_<product>_<event_kind>_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_<product>_<event_kind>_events', INTERVAL '7 days');
SELECT add_retention_policy('meteora_<product>_<event_kind>_events',   INTERVAL '30 days');
```

Adjust compression/retention intervals only if the event class warrants it — otherwise
match the 7d/30d house default.

## Step 3 — GRANTs (same file, end of section)

`setup_roles.sql` default privileges already grant SELECT (+ sequence USAGE) on *future*
tables to all three runtime roles. The existing migrations still list grants explicitly —
**match that style**, granting only what each role needs:

```sql
GRANT SELECT, INSERT, UPDATE ON meteora_<product>_<event_kind>_events TO yog_indexer;
GRANT SELECT                 ON meteora_<product>_<event_kind>_events TO yog_api;
-- add yog_context only if context writes this table
```

Never grant INSERT/UPDATE to `yog_api` — read-only is a deliberate safety net.

## Step 4 — Cross-protocol VIEWs (only if the table feeds one)

VIEWs are first defined in `001_initial_schema.sql` with `CREATE VIEW`. To add a branch you
must **redefine the whole view** in your new migration with `CREATE OR REPLACE VIEW`,
keeping the same column list and order, appending a `UNION ALL` branch that injects the
`protocol` literal:

```sql
CREATE OR REPLACE VIEW swap_events AS
SELECT 'meteora_damm_v2'::TEXT AS protocol, id, pool_address, signature, … , timestamp
  FROM meteora_damm_v2_swap_events
UNION ALL
SELECT 'meteora_<product>'::TEXT AS protocol, id, pool_address, signature, … , timestamp
  FROM meteora_<product>_<event_kind>_events;
```

Protocol-specific columns stay OUT of the VIEW — slim common columns only; code needing the
extras reads the underlying table directly.

## Step 5 — Apply + regenerate SQLx cache

```bash
# Applied as the yog_migrate role (reads DATABASE_URL_MIGRATE from .env):
cargo run --bin yog-migrate -p yog-persistence

# If any sqlx::query!/query_as! macro now hits the new schema, regenerate the
# offline cache or the sqlx-check CI job fails:
cd crates/persistence && cargo sqlx prepare
```

Commit **the new migration AND the updated `crates/persistence/.sqlx/`** together.

## Definition of done

- [ ] `NNN_*.sql` created with the next number; no committed migration edited
- [ ] Table only has protocol-relevant columns; `PRIMARY KEY (id, timestamp)`; hypertable +
      indexes + compression/retention set
- [ ] UNIQUE INDEX on `(signature, timestamp)` for idempotency
- [ ] GRANTs in the same file; `yog_api` stays read-only
- [ ] VIEWs redefined via `CREATE OR REPLACE VIEW` with the new `UNION ALL` branch (if applicable)
- [ ] Migration applied locally via `yog-migrate`; `.sqlx/` regenerated and committed
