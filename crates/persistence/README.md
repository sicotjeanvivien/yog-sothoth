# yog-persistence

PostgreSQL adapter for yog-sothoth. Concrete implementations of the repository
traits declared in `core`, the migration suite, and the one-shot `yog-migrate`
binary that applies it. No business logic lives here — and no other crate
writes SQL.

For the workspace-level picture (dependency graph, conventions, database
roles table, recipes), see [`crates/README.md`](../README.md). For the full
migration conventions, see [`migrations/README.md`](./migrations/README.md).

---

## Layout

```
persistence/
├── migrations/              ← sqlx migrations, forward-only (001 … 040 today)
│   └── README.md            (forward-only convention, GRANT policy, workflow)
├── setup_roles.sql          ← one-time role provisioning (admin)
├── .sqlx/                   ← committed offline query cache (see below)
└── src/
    ├── database.rs          ← Database::connect, run_migrations
    ├── health.rs            ← PgHealthChecker
    ├── repositories/        ← one impl per domain repository trait
    │   ├── helper/          (pubkey/u64/u128 conversions, pagination helpers,
    │   │                     sqlx error mapping)
    │   ├── meteora/damm_v2/ (per-event-kind event repositories — 19 today —
    │   │                     plus the cp-amm pool-properties satellite)
    │   ├── meteora/dlmm/    (the DLMM pool-properties satellite; no event
    │   │                     repositories yet)
    │   ├── pool/, pool_current_state/, pool_analytics/, global_analytics/
    │   ├── signal/, swap_flow/, liquidity_flow/, pool_price_snapshot/
    │   ├── token_metadata/, token_price/, network_status/, watched_pool/
    │   ├── announcement/
    │   └── event_freshness.rs
    ├── bin/migrate.rs       ← yog-migrate binary (~30 lines)
    └── tests/               ← DB-backed integration tests (#[ignore]d by default)
        ├── main.rs          ← the ONLY test target; declares every file below
        ├── helpers.rs       ← shared sentinels (pk, sg, ts)
        └── <subject>.rs     ← one file per event / read path
```

## Repository implementations

One `Pg*Repository` per domain trait. Each takes a `PgPool` in its constructor;
the pool is owned by the consumer — each binary instantiates its own pool with
its own role credentials.

```rust
pub struct PgMeteoraDammV2SwapEventRepository { pool: PgPool }

#[async_trait]
impl MeteoraDammV2SwapEventRepository for PgMeteoraDammV2SwapEventRepository {
    // sqlx::query! / query_as! against self.pool,
    // errors mapped via map_sqlx_error,
    // row → domain conversion via TryFrom<XxxRow> in the sibling rows.rs.
}
```

Row types follow the convention `Row + TryFrom<XxxRow> for XxxDomain`: SQL
types in (`String`, `i64`, `BigDecimal`, …), domain types out (`Pubkey`,
`u64`, `u128`, …). Any parsing failure surfaces as
`RepositoryError::Integrity`. `map_sqlx_error` translates `sqlx::Error`
variants into the right `RepositoryError` semantic (`NotFound`, `Conflict`,
`Timeout`, `Backend`, `Integrity`).

## Per-protocol table strategy ("voie 3")

Each `(protocol, event_kind)` combination has its own SQL table, named
`<platform>_<product>_<event_kind>_events` — nineteen `meteora_damm_v2_*_events`
tables today. Each table holds only the columns relevant to its protocol: no
NULL columns for incompatible fields, no generic JSONB blob. When DLMM or
another protocol lands, it gets sibling tables with their own schemas.

For unified reads, cross-protocol SQL **VIEW**s (`swap_events`,
`liquidity_events`, `claim_position_fee_events`, `claim_reward_events`) expose
the slim common columns plus a synthesised `protocol` column. Protocol-specific
columns are *not* in the VIEWs — code that needs them reads the underlying
table. A VIEW is added only once a second protocol exposes the same concept;
the newer DAMM v2 tables (position lifecycle, pool admin) are read
per-protocol directly.

Cross-protocol concepts stay generic, single-table: `pools`,
`pool_current_state`, `watched_pools`, `network_status`, `token_metadata`,
`token_prices` — and `signals`, where the discrimination is two *columns*
(`detector`, `protocol`), not per-anything tables: a signal is a uniform
conclusion, not a heterogeneous event.

### The unique key of an event table

Every event table carries `slot`, `event_index` and `transaction_index`, and
its idempotency guard is **`(signature, event_index, timestamp)`** — one rule
for all nineteen (migration 041). `timestamp` is in the key because TimescaleDB
requires the partitioning column in a unique index, not to discriminate.

Before that migration the key was `(signature, timestamp)`, which cannot tell
apart the events of a transaction routed across several pools: with
`ON CONFLICT DO NOTHING` and a discarded `rows_affected`, every hop but one was
dropped without an error, a log or a metric — 3,4 % to 8,0 % of a pool's swaps,
the rate rising with how central the pool is to routing. Five tables had grown
their own discriminant (`reward_index`, `second_position`); those columns
remain as data but left the key, because `event_index` is strictly more general
and covers the fourteen kinds that never had one.

Two consequences when you add an event table:

- Give it the three columns and that key — the gabarit in
  `migrations/README.md` has the exact DDL.
- Have its `insert` return `InsertOutcome::from_rows_affected(…)`. Returning
  `Ok(())` re-creates precisely the blindness above.

### The ordering key of the `pool_current_state` projection

The same three columns land on the projection as `last_slot`,
`last_event_index`, `last_transaction_index` (migration 042), and its upsert
guard compares them **as a tuple**:

```sql
WHERE (last_slot, COALESCE(last_transaction_index, 0), last_event_index)
    < (EXCLUDED…)
```

`last_event_at` is still written — it is what `/latest-state` displays — but it
stopped ordering anything. It came from `blockTime`, so a second, and 56 % of
swaps share theirs with another swap of the same pool: ordering on it rejected
**a third of all state updates**, and labelled them `stale` as though they were
healthy concurrency.

Two things to know before touching this query:

- **It is a CTE, not a bare `INSERT`, on purpose.** A guarded `ON CONFLICT`
  returns no row when the guard fails, so the statement could never say *why*.
  The `previous` CTE reads the pre-statement snapshot in the same round-trip,
  which is what lets the upsert report a `same_slot_ambiguity` — and that
  report is a **lower bound** under concurrency, because the guard is
  re-evaluated against the latest committed row while the CTE is not. The
  method's doc-comment carries the detail.
- **Within one slot the order is partial and biased**, not a coin flip:
  `event_index` numbers the emissions of one transaction, so comparing it
  across two ranks unlike things, and the largest index wins. It is kept
  because it is order-independent — a replay reproduces the same final state —
  where last-writer-wins would be unbiased and non-deterministic. gRPC closes
  the gap by filling `transaction_index`, with no further migration.

## Choosing how to write a query

A query-builder/ORM migration (SeaQuery et al.) was evaluated in June 2026 and
**rejected**: it builds SQL at runtime, losing the `query!` compile-time schema
check, and is worse on the CTE/LATERAL queries that actually hurt. Pick by
query shape:

- **Simple / static** → `sqlx::query!` / `query_as!` inline. The default.
- **Big but static** → prefer a **SQL VIEW** in a migration when the query is
  reusable or decomposable (e.g. `meteora_damm_v2_pool_hourly_activity`,
  migration 019, shared by `history` and `pool_analytics`); the slim
  `SELECT … FROM <view>` stays a checked `query!`. Otherwise
  `query_file!("….sql")`.
- **Dynamic** (shape varies from user input) → `QueryBuilder`, covered by
  integration tests. The lone case today is `repositories/pool/query.rs`.

A plain VIEW gives **no** performance gain — Postgres inlines it. Choose a
VIEW for readability; the perf tool is materialization (the hourly continuous
aggregates), which precomputes at the cost of staleness.

## The `yog-migrate` binary

```bash
cargo run -p yog-persistence --bin yog-migrate
```

Reads `DATABASE_URL_MIGRATE`, connects under the `yog_migrate` role, applies
pending migrations via `Database::run_migrations()`, exits 0. In Docker it
runs once at compose-up; runtime services depend on it via
`service_completed_successfully` so they never start against a half-migrated
schema. It is the **only** path through which DDL flows — the five runtime
roles cannot CREATE or ALTER anything.

Note: the migration suite is embedded at compile time (`sqlx::migrate!`) — a
new `.sql` file requires rebuilding the binary to be picked up.

## Migrations

Forward-only: committed migrations never change, no `.down.sql`, rollback is a
backup restore. GRANTs live in the migration that creates the object. Full
conventions and the local workflow: [`migrations/README.md`](./migrations/README.md).

The suite at a glance: `001` consolidated v0.1 baseline → `002`–`008` DAMM v2
position-lifecycle and pool-admin event tables → `009` differentiated
retention/compression → `010`–`013` + `017` hourly continuous aggregates →
`014`–`016` + `018` pool properties resolved by yog-context (mints, fee_bps,
fee split) → `019`–`021` analytic VIEWs (hourly activity, current TVL, valued
liquidity) → `022`–`025` the signal engine (the `signals` hypertable +
`yog_signals` role grants, the hourly swap-flow, price-snapshot and hourly
liquidity-flow read VIEWs) → `026`–`027` announcements and the cp-amm fee-config
columns → `028`–`035` the remaining DAMM v2 event tables (protocol fee, the five
reward instructions, split-position) → `036`–`040` the pool-properties
satellites: cp-amm out of `pools` (`036`–`037`), the `needs_refresh`
invalidation flag (`038`), DLMM (`039`), and the pool↔protocol invariant
(`040`).

### Pool-properties satellites, and the invariant that ties them to `pools`

A satellite table holds the properties that exist for **one** protocol only, so
`pools` stays the cross-protocol registry it was in `001`. Two exist today —
`meteora_damm_v2_pool_properties` (036) and `meteora_dlmm_pool_properties`
(039) — one row per pool, primary-keyed on `pool_address`, owned by
`yog-context`.

Migration `040` writes into the schema a rule that had lived only in application
discipline: **a row hanging off `pools` cannot carry a protocol the registry
disagrees with**. It covers both satellites and `pool_current_state`, which has
duplicated `pools.protocol` in its own column since `001`.

For a satellite the protocol is a constant of the table, so two lines go in its
`CREATE TABLE`:

```sql
protocol TEXT NOT NULL GENERATED ALWAYS AS ('<the protocol>') STORED,
FOREIGN KEY (pool_address, protocol)
    REFERENCES pools (pool_address, protocol) ON DELETE CASCADE
```

— the composite FK **instead of** the single-column one. The generated column
needs no `CHECK` (the constant is the check), no `UPDATE` back-fill, and no
change to any `INSERT`: Postgres refuses to let a writer name it at all. On an
existing table, note that `ADD COLUMN … GENERATED … STORED` rewrites it under
`ACCESS EXCLUSIVE` — free at these row counts, worth sizing at scale. Where the
protocol is real per-row data rather than a constant (`pool_current_state`),
there is no column to add: the FK swap alone.

**This is defence in depth, not a hole being plugged.** The live write paths
already agree with the registry — the resolvers reject a foreign payload, the
`yog-context` worker skips a pool whose decoded account disagrees with the
queue's protocol, and the indexer hands `discover_pool` and the state upsert the
same `Self::PROTOCOL`. What the constraint adds is survival: those guards sit in
loops and call sequences no compiler protects, and hold only for today's
callers. See `tests/pool_properties.rs`, section *The pool↔protocol invariant*.

## `setup_roles.sql`

Slim provisioning script applied once per database as superuser. Creates the
five runtime roles, transfers `public` schema ownership to `yog_migrate`, and
sets `ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate` so tables created by
future migrations inherit the right `SELECT` grants automatically. It contains
no table-specific GRANTs — those live in the migrations. The role → rights →
process mapping is documented in [`crates/README.md`](../README.md#database-roles).

### The privilege matrix is tested

`tests/privileges.rs` declares the intended privilege surface by hand and asserts
it against what the migrations actually produce — in **both** directions, so a
forgotten GRANT and an unintended one both fail. **Adding a table means adding
its line**; the failure prints the exact `GRANT`/`REVOKE` for whatever disagrees.

Read that failure as a question — *is the migration wrong, or the matrix?* —
before editing either. Pasting the missing line to go green is how migration
036's gap survived a month: `yog_indexer` lost the rights it needed when the
fee-shape columns moved to a satellite, and every write it attempted failed with
`permission denied` under its real role, silently, because the writes are
skip-and-log and the tests run as the owner.

Scope: **explicit grants only**. The default privileges above do not reproduce in
a `sqlx::test` database — that file is not a migration, and `ALTER DEFAULT
PRIVILEGES FOR ROLE yog_migrate` only covers objects *created by* `yog_migrate`,
while tests apply migrations as the connecting user. The module doc spells out
what that leaves uncovered.

## SQLx offline cache

The crate uses `sqlx::query!` macros verified against the live schema at
compile time. The verified cache is committed under
`crates/persistence/.sqlx/`, which lets the workspace build everywhere with
`SQLX_OFFLINE=true`.

**After modifying any `sqlx::query!` call**, regenerate the cache before
committing — CI runs `cargo sqlx prepare --check` against a real Postgres:

```bash
cd crates/persistence
cargo sqlx prepare
```

## Integration tests

DB-backed tests live in `tests/` and are `#[ignore]`d by default:

```bash
cargo test -p yog-persistence --features integration-tests -- --include-ignored
```

They need a live Postgres running with `timescaledb.max_background_workers = 0`
(as configured in `docker-compose.yml`): `sqlx::test` creates a fresh database
per test, and the cagg refresh policies from migrations 010–013 otherwise have
the TimescaleDB job scheduler race the next test's migration DDL on the shared
catalog ("tuple concurrently deleted").

**One file per subject, one single binary.** Cargo auto-discovers every `.rs`
directly under `tests/` as its own test target, which relinks the whole crate
once per file. `autotests = false` in `Cargo.toml` turns that off and declares
`tests/main.rs` as the only target (`--test integration`); every other file is a
plain module of it, and `tests/helpers.rs` holds the sentinels they share
(`pk`, `sg`, `ts`). Adding a test file means creating `tests/<subject>.rs` **and
declaring it in `tests/main.rs`** — without that `mod` line it compiles to
nothing and runs silently.

---

## `watched_pools` — startup allowlist

Until the indexer runs on an upgraded RPC path (Helius `transactionSubscribe`
or a managed Yellowstone gRPC stream), ingestion is bounded by an allowlist of
pools stored in the `watched_pools` table. The protocol-centric architecture is
preserved — the allowlist bounds **what the indexer subscribes to**, not what it
accepts once received: under `MODE_PROTOCOL_CENTRIC=false` the listener opens
one `logsSubscribe` per active row instead of one per program id
(`WatchedPoolService::restore_subscriptions`). Nothing downstream is aware of
it — no filter, no code path conditioned on a pool list — so lifting the
constraint is a config flip, not a return from static configuration.

The rationale is summarised in the
[root README's *Pool observation model*](../../README.md#pool-observation-model).
The content below is the operational reference.

### Schema

| Column | Type | Purpose |
|---|---|---|
| `pool_address` | `TEXT PRIMARY KEY` | Solana pubkey of the pool |
| `protocol` | `TEXT NOT NULL` | Protocol identifier (`damm_v2`, etc.) |
| `active` | `BOOLEAN NOT NULL DEFAULT TRUE` | Whether the pool gets a subscription at startup |
| `added_at` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` | When the pool was added to the allowlist |
| `note` | `TEXT` | Free-form annotation (selection rationale, edge-case marker, etc.) |

A partial index on `(pool_address) WHERE active = TRUE` keeps the lookup cheap
regardless of how many deactivated rows accumulate over time.

Deactivation uses the `active` flag rather than row deletion, to preserve
history and allow reactivation without re-selection.

### Decoupling from `pools`

There is **no foreign key** from `watched_pools.pool_address` to
`pools.pool_address`. The two tables serve different purposes:

- `pools` is a **record** — what the indexer has observed in the transaction stream.
- `watched_pools` is a **configuration** — what the indexer is authorised to ingest.

A pool can legitimately appear in `watched_pools` before it appears in `pools`
(the moment between seeding the allowlist and observing the first transaction).
Forcing a FK would either reject the seed or require pre-populating `pools`
with empty rows, both worse than the current decoupling.

### Current selection

The allowlist was seeded from the 7-day activity distribution of `swap_events`
observed during a calibration window in **April 2026** — the timestamps below
are that window's, not a live picture. Pools were chosen to balance
high-signal density (top of the distribution) with edge-case diversity
(lower-activity pools for testing short-lived or thin-liquidity behaviour).

| Pool address | 7d swap count | First swap (UTC) | Last swap (UTC) | Notes |
|---|---:|---|---|---|
| `AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv` | 906 | 2026-04-22 09:02 | 2026-04-22 09:53 | High activity, short burst |
| `8DW1L4yJRm2NNygASN1nFKEXwxLurkozxuYATZCT3gpb` | 818 | 2026-04-22 09:31 | 2026-04-22 09:53 | High activity, short burst |
| `9g2wf7xTBsVxoVnypCdKrUmBtH6Ms1tSzVEJQNj86eHg` | 774 | 2026-04-22 09:43 | 2026-04-22 09:53 | High activity, very short window |
| `5BohNRJgMtSv9C4PqxhvkXL1v1j7gouBoj4usNG8LGH` | 758 | 2026-04-22 09:31 | 2026-04-22 09:53 | High activity, short burst |
| `GpnMyz78yTRiS2oBMroEKEynG7LkjWZq61aaU1MD558L` | 720 | 2026-04-21 09:24 | 2026-04-21 09:59 | High activity, previous day |
| `6bkGH5bdNWym7eP2KKDDbCt5jMn9NB1dV7dN9fbb1Bz8` | 674 | 2026-04-22 09:43 | 2026-04-22 09:53 | High activity, very short window |
| `CfpwKVuB8Y41re9U5qpYmD3oYiDijTcsHe3c3fs8GsFg` | 601 | 2026-04-22 12:23 | 2026-04-22 12:23 | Extreme burst (<1 min) |
| `AMxysMpo34c3aNb5bWW28p4AkXzWJFdM5Wdrtfmy4bMx` | 237 | 2026-04-21 09:59 | 2026-04-21 09:59 | Ephemeral, edge case |
| `EV9h8xS1yF3GJ8LnkaE65hQx5ViCSSeoVaHT6JPaVyPW` | 235 | 2026-04-21 09:24 | 2026-04-21 09:33 | Ephemeral, edge case |
| `59drqEGrECHxMkHPKcr1JZggNfPxNKsrQP5MvCBEY5av` | 234 | 2026-04-21 09:41 | 2026-04-21 09:42 | Ephemeral, edge case |

> **Note on observed activity patterns** — most pools in the selection exhibit
> burst behaviour (high swap count over a short window, then quiescence). This
> is consistent with DAMM v2 being used heavily for memecoin launches.
> Longer-lived pools will be added as the dataset grows.

### Seeding the allowlist

⚠️ **There is no seed script in the repo.** This README and
[`crates/README.md`](../README.md) both used to point at
`scripts/seed_watched_pools.sql`; that file was never written, so the step was
undocumentable as stated. The table above is the selection to reproduce, and
the seed is an `INSERT` you run by hand:

```bash
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" <<'SQL'
INSERT INTO watched_pools (pool_address, protocol, note) VALUES
    ('<pubkey>', 'damm_v2', 'high activity, short burst')
ON CONFLICT (pool_address) DO NOTHING;
SQL
```

`ON CONFLICT DO NOTHING` keeps it idempotent — safe to re-run after a partial
seed or against an existing database.

Run it as the admin role rather than as `yog_indexer`: the seed adjusts the
allowlist, which is configuration, not runtime data, and the convention is to
keep all configuration writes under the admin role.

Without at least one active row, an indexer started in pool-centric mode has
nothing to subscribe to and exits on `NoSubscriptionTargets` — this step is not
optional.

### Administration helpers

These are the four operations you'll run by hand to manage the allowlist
ad-hoc. They are intended for the admin role:

```sql
-- Add a pool
INSERT INTO watched_pools (pool_address, protocol, note)
VALUES ('<pubkey>', 'damm_v2', 'manual selection: high TVL');

-- Deactivate without losing history
UPDATE watched_pools
SET active = FALSE
WHERE pool_address = '<pubkey>';

-- Reactivate
UPDATE watched_pools
SET active = TRUE
WHERE pool_address = '<pubkey>';

-- List currently active
SELECT pool_address, protocol, added_at, note
FROM watched_pools
WHERE active = TRUE
ORDER BY added_at DESC;
```

The allowlist is read once at indexer startup, when the subscriptions are
opened. Modifying `watched_pools` while the indexer is running has no effect on
the running process — restart the indexer to pick up the change. Hot reload
becomes relevant in **v0.3** when user-managed watchlists arrive as a
first-class feature.

### Removing the constraint

The allowlist is temporary. It will be lifted once one of the following is in
place:

- **Helius `transactionSubscribe` (Developer plan)** — eliminates the HTTP
  fetch entirely; transactions arrive fully parsed in the WebSocket stream.
- **Helius Startup Launchpad** — 8 months of Business tier free (LaserStream
  mainnet, 200 RPS).
- **A managed Yellowstone gRPC (Geyser) provider** (Shyft, Triton, …) with
  matching throughput.

At that point the indexer switches to `MODE_PROTOCOL_CENTRIC=true` — one
subscription per program id — and ingestion returns to full protocol-centric
coverage. The `watched_pools` table stays in the schema; it simply stops being
read, becoming purely informational rather than enforced.

---

## See also

- [`crates/README.md`](../README.md) — workspace architecture, database roles, recipes
- [`migrations/README.md`](./migrations/README.md) — migration conventions (forward-only, GRANTs per migration, local workflow)
- [Root README](../../README.md) — project pitch, roadmap, getting started
