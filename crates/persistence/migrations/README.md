# Migrations — yog-persistence

Applied by the `yog-migrate` binary at startup of the Docker compose
stack, and manually during development via:

```sh
cargo run --bin yog-migrate -p yog-persistence
```

The connection string passed to `yog-migrate` must use the `yog_migrate`
role — the four runtime roles (`yog_indexer`, `yog_api`, `yog_context`,
`yog_signals`) intentionally cannot CREATE or ALTER tables.

## The baseline, and where the old numbers went

`001_baseline.sql` is the whole schema. It replaces the 42 files that built it
incrementally between June and August 2026, squashed on **5 August 2026** —
before the first production deployment, the only window in which a squash is
free. The next migration is `002_`.

`002_swap_implied_price.sql` — values a swap bucket by whichever of its two
tokens is priced, and is the one place where a cagg has been dropped and
rebuilt. That was free precisely because no cagg had ever materialized a bucket
(the job scheduler has been off since 16 June); it will not be free again once
the scheduler runs. A cagg cannot be `ALTER`ed, so any future column added to
one costs the same drop — and by then, a backfill.

`003_drop_pool_current_state_liquidity.sql` — drops `pool_current_state.liquidity`
and `last_liquidity_at`. The first named the pool's concentrated-liquidity **L**
and held a single position's unsigned `liquidity_delta`, last-write-wins; the
second was exact but read by nobody. **A DROP COLUMN is forward-only like any
other migration**: the data is not recoverable from the schema afterwards, so
the justification belongs in the file header, and it is there. Nothing was lost
here — the same magnitude stays on the liquidity event rows, next to the
add/remove kind that gives it meaning.

`004_fee_scheduler_params.sql` — six columns on the cp-amm satellite holding a
fee scheduler's decay curve, so a pool's base fee can be evaluated at read time
instead of being frozen on its genesis cliff (ticket 07, measured wrong by ×5
and ×49). They are NULL for every fee shape that has no time curve, and that
NULL is a decoded fact: `BaseFeeInfo` is 32 bytes the modes reinterpret, so the
same offsets under a market-cap scheduler or a rate limiter yield
plausible-looking nonsense. **No GRANT** — this satellite is granted at table
level, unlike `pools` whose `yog_context` rights are column-scoped.

`005_price_staleness_policy.sql` and `006_flow_valuation_completeness.sql` are
not described here: they redefine views (and, for 005, add the two price-age
functions) without changing any table. Their headers carry the reasoning, and
nothing this file could add would beat reading them.

`007_referral_fee_split.sql` — **the second drop and rebuild of the swap cagg**,
for two columns (`referral_fee_in_a` / `referral_fee_in_b`) that let the
realized fee be split into the three shares cp-amm actually applies (ticket 05:
the LP share was published as `fees − protocol`, which credits the referral to
the LPs). Still free, for the same reason 002 was — no bucket has ever been
materialized. ⚠️ **That is the point to carry forward, not the fix**: two
migrations have now paid nothing for a rebuild, and 002 had the window open in
August without adding these columns. The next one will need a backfill. A cagg
cannot be `ALTER`ed, so a column you can foresee wanting belongs in the rebuild
you are already doing.

⚠️ **`001_baseline.sql` §13 still states the pre-007 rule as current fact** —
"so the LP share is `(fee_in_x - protocol_fee_in_x)`", in the header of this
very aggregate. It is wrong, and forward-only means it cannot be edited. That
matters more than a stale comment usually would: by this file's own argument
(*"a comment that says 'see migration 036' still resolves"*), the baseline is
where people go to read the current shape of an object — so the next reader of
the swap cagg meets, first, the formula 007 exists to remove. The correct rule
is in `007`'s header and in `crates/persistence/README.md` → *The realized fee
split*. This is the same defect the ticket is about, one level up: one
definition, written in two places, one of which went stale.

`008_cagg_refresh_below_retention.sql` — moves the four refresh policies from
`start_offset = 31 days` to `29`, against an unchanged `drop_after = 30 days`.
No view is touched and no aggregate rebuilt, so the window above is not spent.

## The rule that binds a refresh policy to a retention policy

> **`start_offset` must be STRICTLY smaller than `drop_after`.**

A refresh must never look at a range whose raw rows may already be gone.
`drop_chunks` logs an invalidation over what it removes; a refresh is
invalidation-driven, so a window containing that invalidation recomputes the
range from rows that no longer exist and writes back the nothing it finds —
**deleting** the materialized buckets. The retention never touches the
aggregate; the refresh does.

⚠️ **The 7-day chunk geometry does not protect you.** A chunk is dropped only
once entirely older than `drop_after`, which *looks* like it keeps the dropped
range clear of a window that only overshoots by a day. It does not, because
retention runs **daily**: a chunk is dropped at the first run after its end
crosses the line, so its newest rows are then between 30 and 31 days old —
inside a 31-day window. Measured 10 August 2026 on the real geometry:
**2160 materialized buckets → 2136, exactly 24 — one day of history per chunk
dropped**, permanently, about one day in seven beyond the 30-day line.

Both directions are asserted by `tests/cagg_retention.rs`: the rule itself, read
out of the TimescaleDB catalog for all four pairs, and the behaviour it exists
for. ⚠️ `001_baseline.sql:1664` still carries the reasoning that produced the
bug — *"start_offset spans the full 30d retention window (raw rows never live
longer)"* — and forward-only means it cannot be corrected there.

## Watching the job scheduler run, locally

`docker-compose.yml` pins `timescaledb.max_background_workers = 0`, and it has
to stay there: `sqlx::test` creates a database per test and the scheduler races
the next test's DDL on the shared catalog. So no cagg had ever materialized a
bucket, and no policy had ever run — which is why every finding above went
unnoticed. To look, without committing anything:

```bash
cat > /tmp/scheduler-on.yml <<'YML'
services:
  postgres:
    command: ["postgres", "-c", "timescaledb.max_background_workers=8"]
YML
docker compose -f docker-compose.yml -f /tmp/scheduler-on.yml up -d postgres
# … observe, then put it back:
docker compose up -d postgres
```

What to look at — `timescaledb_information.job_stats` joined to `jobs` for
`total_runs` / `total_failures`, and the materialization watermark:

```sql
SELECT ca.view_name,
       to_timestamp(_timescaledb_functions.cagg_watermark(h.id) / 1000000.0)
  FROM timescaledb_information.continuous_aggregates ca
  JOIN _timescaledb_catalog.hypertable h
    ON h.table_name = ca.materialization_hypertable_name;
```

Done on 10 August 2026: all four refresh policies ran and succeeded on the first
scheduler tick, and the swap aggregate materialized **185 buckets** — the first
time any of them has. `claim_reward_events_hourly` stayed at `-infinity`, which
is correct: its raw table holds no rows. ⚠️ Note the consequence for the
free-rebuild window above — it is closed on any database where the scheduler has
now run, this dev one included.

The point was not to have fewer files. It was that the current shape of a table
had stopped being readable anywhere: `pools` had to be reconstructed by reading
001 + 014 + 015 + 018 + 027 + 036 + 037 + 038 and replaying the ADD/DROPs
mentally.

**A comment that says "see migration 036" still resolves.** The baseline is cut
into numbered sections, each naming the migrations it absorbs, and its header
carries the full old-number → section table. Grep the old number in
`001_baseline.sql`. The original files stay in git history
(`git log -- crates/persistence/migrations/`).

### How the squash was proved equivalent

Two fresh databases were built — one from the 42 migrations, one from the
baseline — and compared on **two** axes, because neither alone is sufficient:

1. `pg_dump --schema-only --schema=public`, with the order-dependent
   TimescaleDB internal ids normalised and the blocks sorted (pg_dump emits in
   dependency order, so the same schema built by two statement sequences yields
   permuted dumps);
2. the TimescaleDB catalog — `timescaledb_information.{hypertables, dimensions,
   compression_settings, continuous_aggregates, jobs}`. Retention, compression
   and refresh policies are catalog **rows**, not DDL, and pg_dump does not emit
   them.

Both empty. Mutation-tested in both directions: removing one retention policy
from the baseline produces **0** lines of pg_dump diff and 2 lines of catalog
diff; removing one column produces 2 and **0**. A dump-only comparison would
have been green while silently dropping every policy.

One deliberate difference was folded in — see below.

### ⚠️ What that comparison still cannot see: default privileges

Both oracle databases had `setup_roles.sql` applied, so `ALTER DEFAULT
PRIVILEGES` granted SELECT to the runtime roles on every new table and view —
and **an ACL cannot tell a default-privilege grant from an explicit one**. A
lost explicit `GRANT SELECT` is therefore invisible to the schema diff.

This is not hypothetical: migration 014 dropped and recreated `swap_events` and
`liquidity_events` without restating the grant 001 had given them, and nothing
noticed for two months. What caught it during the squash was
`tests/privileges.rs`, whose `sqlx::test` databases have no default privileges
— it pins the real matrix and it went red.

The grant is restored in the baseline (§14), which makes that section the one
place this file is not a pure squash. Folding it in rather than adding a `002_`
is defensible only because it happened **before the first deploy**, and only
because the privilege surface has its own assertion: the schema diff is blind
here, the matrix is not. After the first deploy the same finding would have to
go forward in a new migration.

Run the integration suite, not just a schema diff.

### ⛔ A database created before 5 August 2026: recreate it, do not resync

If your database was migrated by the 42-file chain, it holds a `_sqlx_migrations`
row for version 1 whose checksum is `001_initial_schema.sql`'s, plus versions 2
to 42 that no longer exist on disk. `yog-migrate` will refuse it —
`migration 1 was previously applied but has been modified` — and it is right to.

**The only correct action is to recreate the database.** Locally that is
`DROP DATABASE` then `yog-migrate -- bootstrap`; the data is disposable, which
is the whole reason the squash was allowed to happen at all.

⚠️ **Do not reach for the checksum-resync recipe below.** It is the first thing
this file offers a reader hitting a checksum error, and here it is actively
harmful: it would stamp the baseline's checksum onto a row recording a
*different* file, and leave versions 2-42 claiming migrations that the resolved
set no longer contains. The database would report itself up to date while its
recorded history describes a schema nobody can reproduce. That recipe exists for
one narrow case — a committed file edited in ways an applied database provably
never evaluated — and this is not it.

## Forward-only

Migrations are forward-only. **A migration committed to git never
changes.** No `.down.sql` files; no edits to past migrations. If a
released migration introduced a problem, fix it forward by writing a
new migration that corrects the state.

**The one window where editing is admissible, and its price.** The rule
protects databases that have applied a migration and hold data you cannot lose.
Before the first production deploy that set is *only* developer machines, whose
data the project already treats as disposable. Editing is then a judgement
call, not a violation — but it is never free, and the cost falls where it is
least visible:

- CI recreates a database per run, so **CI stays green and proves nothing**;
- the failure surfaces only on machines that already applied it, later, as
  `migration N was previously applied but has been modified`, looking like a
  regression.

So an edit owes the team two things in the same commit: a statement of what
changed *in executable SQL* (ideally nothing), and the exact repair.

The repair re-syncs every recorded checksum with the file on disk. Prefer this
over a per-version `UPDATE`: it is idempotent, and it does not need to know
which migrations moved — which matters when two branches each touched a
different set.

```sh
python3 - "$DATABASE_URL_MIGRATE" <<'PY'
import hashlib, pathlib, re, subprocess, sys
stmts = []
for f in sorted(pathlib.Path("crates/persistence/migrations").glob("*.sql")):
    v = int(re.match(r"(\d+)_", f.name).group(1))
    c = hashlib.sha384(f.read_bytes()).hexdigest()
    stmts.append(f"UPDATE _sqlx_migrations SET checksum = decode('{c}','hex') WHERE version = {v};")
subprocess.run(["psql", sys.argv[1], "-q", "-c", "\n".join(stmts)], check=True)
PY
```

Only run it when the executable SQL is unchanged, or changed in a way an
already-migrated database provably never evaluated. Otherwise the row would
claim a statement ran that never did — recreate the database instead. **In
particular, never run it on a database predating the squash** — see the section
above. Prove it rather than assert it; comparing the files with comment lines
stripped is enough:

```sh
diff <(git show <before>:path/to/0NN_x.sql | grep -vE '^\s*--') \
     <(grep -vE '^\s*--' path/to/0NN_x.sql)
```

Done twice in August 2026, on files the baseline has since absorbed, to put
their headers into English (the repository's convention for migration bodies)
after they had merged. Both times the diff above came back empty or limited to
`RAISE EXCEPTION` strings that a successful run never evaluates.

The squash itself is the largest instance of this window ever taken, and it
closes it: from `002_` onwards there is production data behind the rule.

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
sequences default) live in
[`../src/bin/scripts/setup_roles.sql`](../src/bin/scripts/setup_roles.sql) —
next to the binary that embeds it, applied under the admin role by
`yog-migrate -- setup-roles`.

### An event table carries its position in the chain

Every `*_events` table carries three columns locating it in the chain, and one
uniform idempotency key — the shape migration 041 imposed, now `001_baseline.sql`
§12. A new event table is born with them:

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

The database must already exist (`createdb`, or the compose volume); the binary
never creates one. Everything after that is one command:

```sh
cargo run --bin yog-migrate -p yog-persistence -- bootstrap
```

which is these three, in order:

| step | script | role |
|---|---|---|
| `setup-roles` | [`../src/bin/scripts/setup_roles.sql`](../src/bin/scripts/setup_roles.sql) — the five roles and the structural privileges | `DATABASE_URL_ADMIN` |
| `migrate` | this directory | `DATABASE_URL_MIGRATE` |
| `seed-watched-pools` | [`../src/bin/scripts/setup_watched_pools.sql`](../src/bin/scripts/setup_watched_pools.sql) — the startup allowlist | `DATABASE_URL_ADMIN` |

All three are idempotent, so `bootstrap` is safe to re-run — and safe against a
second database of the same cluster, where the roles already exist and only the
per-database privileges need applying. `bootstrap` checks both environment
variables before touching anything: failing on a missing `DATABASE_URL_MIGRATE`
*after* creating five cluster-wide roles leaves a half-provisioned cluster.

Then the runtime services (indexer / api / context / signals) can connect with
their respective roles — but check the allowlist first. It ships one pool, and
a pool-centric indexer collects only what it is subscribed to.