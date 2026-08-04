# Migrations — yog-persistence

Applied by the `yog-migrate` binary at startup of the Docker compose
stack, and manually during development via:

```sh
cargo run --bin yog-migrate -p yog-persistence
```

The connection string passed to `yog-migrate` must use the `yog_migrate`
role — the four runtime roles (`yog_indexer`, `yog_api`, `yog_context`,
`yog_signals`) intentionally cannot CREATE or ALTER tables.

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

### An event table carries its position in the chain

Since migration 041 every `*_events` table has three more columns and one
uniform idempotency key. A new event table is born with them:

```sql
CREATE TABLE meteora_<product>_<event_kind>_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,
    -- … protocol-relevant columns …
    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

-- Idempotency guard. `event_index` is what separates two events emitted by
-- ONE transaction — a route across several pools emits one per hop, under a
-- single signature and a single blockTime. Without it, `ON CONFLICT DO
-- NOTHING` silently keeps the first and drops the rest.
CREATE UNIQUE INDEX ON meteora_<product>_<event_kind>_events
    (signature, event_index, timestamp);
```

No `DEFAULT` on `slot` / `event_index`: an insert that forgets one must fail,
not inherit a plausible `0`. Widths follow the domain type, not the expected
magnitude — `event_index` is a `u16` so it is INTEGER, `transaction_index` a
`u32` so it is BIGINT (see `convert_i32_to_u16`'s doc-comment); both write
conversions are then total.

Do **not** invent a per-kind discriminant (`reward_index`, `second_position`
and friends). Those predate `event_index`, and migration 041 took them out of
the keys precisely to leave one rule.

## The compression WARNINGs are expected

Enabling compression on an event hypertable makes TimescaleDB emit, once per
table:

```
WARNING:  column "id" should be used for segmenting or ordering
WARNING:  column "signature" should be used for segmenting or ordering
```

They appear in bulk during the integration suite, because `sqlx::test`
provisions a freshly migrated database per test and every `ALTER TABLE … SET
(timescaledb.compress, …)` warns again. Nothing is wrong.

**What it means.** Our unique indexes cover columns — `id` through the primary
key, plus `signature` and `event_index` — that are in neither
`compress_segmentby` (`pool_address`) nor `compress_orderby` (`timestamp`).
TimescaleDB warns that it cannot check those constraints against compressed
rows *cheaply*.

**It says "should", not "cannot": uniqueness is still enforced.** Verified
empirically on TimescaleDB 2.27 — inserting a duplicate into a compressed chunk
raises `duplicate key value violates unique constraint`, and the
`ON CONFLICT … DO NOTHING` idempotency guard behaves exactly as on an
uncompressed chunk. TimescaleDB decompresses the candidate segments to check.
So the warning is about **cost**, not correctness.

⚠️ *If you re-test this, use a fixed timestamp literal.* Two statements each
calling `now()` produce two different timestamps, so a
`(signature, event_index, timestamp)` index sees no conflict at all — a probe
written that way "proves" a hole that does not exist.

**Why we leave it alone.** The compression policy is 7 days and the indexer is
live: events carry a current timestamp and always land in the open, uncompressed
chunk. The expensive path is never taken. Putting `signature` into
`compress_segmentby` would silence the warning at the cost of destroying the
compression ratio — it is maximum-cardinality, so each segment would hold a
single row.

**When it would start to cost.** Backfilling events older than the compression
delay — every insert would decompress to check uniqueness. Correct, but slow.
Worth remembering the day historical replay becomes possible (see the gRPC
migration in the project tracker).

## ⚠️ What a local run cannot prove — compressed chunks

**A migration that passes locally has never met a compressed chunk, and cannot.**
The local Postgres runs with `timescaledb.max_background_workers = 0`
(`docker-compose.yml`, for the reason in `CLAUDE.md`), so the compression
policies never fire: **0 compressed chunk out of 45** on a typical dev
database. Production compresses at 7 days.

This is a structural blind spot, not an accident of timing — raising the worker
count re-introduces the job-scheduler race that made the integration suite
flaky. It matters because compressed chunks change what DDL and DML are allowed:
`ADD COLUMN`, `UPDATE`, and `CREATE UNIQUE INDEX` all have their own rules, and
a migration that touches rows (a backfill) is precisely the kind that could
fail there and nowhere else.

Verify by hand when a migration adds a column, backfills rows, or creates an
index — restore a database at the previous migration, compress its chunks, then
apply:

```sh
psql "$DATABASE_URL_MIGRATE" -c "
    SELECT compress_chunk(c)
    FROM show_chunks('meteora_damm_v2_swap_events') c;"
# then apply the migration and re-check:
psql "$DATABASE_URL_MIGRATE" -c "
    SELECT count(*) FROM timescaledb_information.chunks WHERE is_compressed;"
```

Done for migration 041 (August 2026): `ADD COLUMN`, the `row_number()` backfill
and `CREATE UNIQUE INDEX` all pass on a compressed chunk, the chunk stays
compressed, and the new unique index rejects a duplicate inserted into it — so
the constraint bites on compressed data too.

Three false greens have already come out of this project by trusting a check
that could not fail: an empty table, an unquantified loss, and this one. **A
verification that cannot come out red is not a verification.**

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
# 1. As the superuser, declare the five roles + structural privileges.
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f crates/persistence/setup_roles.sql

# 2. Apply all migrations as yog_migrate.
cargo run --bin yog-migrate -p yog-persistence
```

After step 2, the runtime services (indexer / api / context / signals)
can connect with their respective roles.