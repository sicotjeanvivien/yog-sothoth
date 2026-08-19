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
├── migrations/              ← sqlx migrations, forward-only (001_baseline.sql today)
│   └── README.md            (forward-only convention, GRANT policy, workflow)
├── .sqlx/                   ← committed offline query cache (see below)
├── src/
│   ├── database.rs          ← Database::connect, run_migrations, run_script
│   ├── health.rs            ← PgHealthChecker
│   ├── repositories/        ← one impl per domain repository trait
│   │   ├── helper/          (pubkey/u64/u128 conversions, pagination helpers,
│   │   │                     sqlx error mapping)
│   │   ├── meteora/damm_v2/ (per-event-kind event repositories — 19 today —
│   │   │                     plus the cp-amm pool-properties satellite)
│   │   ├── meteora/dlmm/    (the DLMM pool-properties satellite; no event
│   │   │                     repositories yet)
│   │   ├── pool/, pool_current_state/, pool_analytics/, global_analytics/
│   │   ├── signal/, swap_flow/, liquidity_flow/, pool_price_snapshot/
│   │   ├── token_metadata/, token_price/, network_status/, watched_pool/
│   │   ├── announcement/
│   │   └── event_freshness.rs
│   └── bin/
│       ├── migrate.rs       ← yog-migrate binary (migrate / setup-roles /
│       │                      seed-watched-pools / bootstrap)
│       ├── migrate/
│       │   ├── lint.rs      ← test-only: the rules yog-migrate imposes on the
│       │   │                  migrations it applies (see below)
│       │   └── tests/
│       │       └── lint_tests.rs
│       └── scripts/         ← the provisioning SQL, `include_str!`d into it
│           ├── setup_roles.sql         (roles + structural privileges, admin)
│           └── setup_watched_pools.sql (startup allowlist seed, admin)
└── tests/                   ← DB-backed integration tests (feature `integration-tests`)
    ├── main.rs              ← the ONLY test target; declares every file below
    ├── helpers.rs           ← shared sentinels (pk, sg, ts)
    └── <subject>.rs         ← one file per event / read path
```

### `bin/migrate/lint.rs` — the rules `yog-migrate` imposes on its migrations

DB-free unit tests over `migrations/*.sql`, chiefly one rule: **every index is
named**. Postgres truncates a generated index name to 63 bytes, our table names
leave almost nothing for the suffix, and two already truncate onto the same name
— so the second one carries a `1` that would move if a third table joined them.
Naming the index removes the question rather than answering it; the same applies
to `create_hypertable`, which is asked for `create_default_indexes => FALSE` so
its default index is written out too.

```bash
cargo test -p yog-persistence --bin yog-migrate
```

They sit with the binary rather than in the library because `migrations/` is
`yog-migrate`'s subject — this crate binds the processes to the database, that
binary owns how the schema changes — and they stay *tests* rather than a runtime
check: a migration that breaks a naming convention is badly written, not
dangerous to apply, so the sanction is a red pull request, not a blocked
deployment.

They bind migrations from `010` on. `001`–`009` are frozen history and break the
rule 70 times; that count is asserted, so the boundary is a fact rather than an
intention. The rule itself, with the naming budget, is in
[`migrations/README.md`](migrations/README.md).

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
for all nineteen (baseline §12). `timestamp` is in the key because TimescaleDB
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
`last_event_index`, `last_transaction_index` (baseline §4), and its upsert
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
  baseline §15, shared by `history` and `pool_analytics`; or
  `meteora_damm_v2_swap_events_hourly_priced`, migration 002, which factors the
  valuation rule out of it); the slim `SELECT … FROM <view>` stays a checked
  `query!`. Otherwise `query_file!("….sql")`.
- **Dynamic** (shape varies from user input) → `QueryBuilder`, covered by
  integration tests. The lone case today is `repositories/pool/query.rs`.

  It builds two queries that must agree: the page, and the `touched_since`
  count beside it. Both go through the same `push_filters`, on purpose — the
  count means "how many pools *the reader is looking at* have left the
  traversal", so a copy that drifted would report pools they filtered out.
  That module also owns the **snapshot fence** (`as_of`) which makes a keyset
  cursor legal over `last_seen_at`, a column the indexer rewrites on every
  event; the contract is on `yog_core::domain::PoolPage`, and the fence is
  minted in `find_paginated` rather than by callers so it cannot be omitted.

A plain VIEW gives **no** performance gain — Postgres inlines it. Choose a
VIEW for readability; the perf tool is materialization (the hourly continuous
aggregates), which precomputes at the cost of staleness.

⚠️ **Inlining is not deduplication.** A view that reads a table its *caller*
also reads gets scanned separately: the plan then holds two scans of the same
hypertable. Migration 002 hit exactly that — the effective-price view was first
joined *alongside* the swap cagg, and `/api/stats` scanned the swap hypertable
twice. The fix is for the view to carry the base columns through so the caller
selects from it alone.

⚠️ **And a CTE referenced twice is materialized, which stops predicates from
descending.** Since Postgres 12 a `WITH` is inlined only when referenced *once*;
past that it is materialized, and the caller's `WHERE` is applied *after*.
`meteora_damm_v2_pool_hourly_activity` is in that case today — its four CTEs are
each read twice (once by the `buckets` UNION, once by the final `LEFT JOIN`), so
a single-pool read aggregates the whole swap hypertable and filters afterwards.
Known and ticketed, not yet fixed.

So the check on a new view over a cagg is **two** things, and the second is the
one that gets forgotten:

```sql
EXPLAIN (COSTS OFF) SELECT … FROM <view> WHERE pool_address = '…' AND bucket > …;
```

1. count the `_hyper_*_chunk` scans — more than one means the base table is read
   twice;
2. check **where the predicate lands**. `Filter:` on a `CTE Scan` means it did
   not descend and the aggregate ran over everything; you want the condition on
   the chunk scan itself.

## USD valuation: which price, and what "unknown" means

Two conventions coexist, and mixing them is a real risk that nothing in the
schema prevents. The rule, written down here because the audit of 3 August 2026
found it stated nowhere:

| kind | what it measures | price used |
|---|---|---|
| **stock** — TVL, reserves, composition | a balance at an instant | the **latest known** price |
| **flow** — volume, fees, liquidity moved | what happened over a window | the **trade-time** price, as of each hourly bucket |

This is the standard practice and each is right for its own quantity, but the
two are **not interchangeable**. ⚠️ A ratio that crosses them is wrong even
though both halves are correct: a "turnover = volume / TVL" — a natural product
ask — would divide a trade-time numerator by a current-price denominator. If
such a metric is ever added, pick one convention for both halves.

### How old a price may be (migration 005)

A price observation has a **validity window**; outside it there is no price, only
a NULL. The two conventions above take the bound from a different reference
point, and confusing the two is how it gets written wrong:

| convention | bound | measured against |
|---|---|---|
| **stock** / latest | `yog_price_max_age_latest()` — 15 min | `now()` |
| **flow** / as-of | `yog_price_max_age_asof()` — 1 h | the bucket's start, or the event's own timestamp |

⚠️ The as-of bound is **never** measured against `now()`. A bucket from ten days
ago valued by a price from ten days ago is correct; bounding it on the present
would erase the whole history.

⚠️ "1 h" is measured from the bucket's **start**, so for `[10:00, 11:00)` the
accepted window is `[09:00, 10:00]` — the hour *before* the bucket, never inside
it. A trade at 10:59 can therefore carry a price 1 h 59 m older than itself, and
a price fetched at 10:30 is rejected. Up to two bucket widths per event, not one.

⚠️ **An as-of gap never heals.** The latest bound recovers on the next
`yog-context` tick; the as-of one does not — the worker only inserts at
`fetched_at = now()` and there is no backfill, so a context outage leaves a
*permanent* hole in `volume_usd` / `fees_usd` / `liquidity_*_usd` for the buckets
it spans. Intended (a wrong number is worse than an absent one, and the coverage
counters surface the absence), but it is a trade, not a free win.

Both intervals are SQL functions, declared once in migration 005 and called from
every valuation view. They are `IMMUTABLE`, so the planner folds them and the
bound becomes an *index condition* on `idx_token_prices_mint_recent` rather than
a filter. **Do not inline the interval literal** — the rule previously lived at
one site out of seventeen, which is exactly how it got lost.

Two consequences worth knowing before you touch a valuation view:

- `pool_current_tvl` goes NULL when `yog-context` stops refreshing prices, so a
  dead enrichment loop makes the TVL *disappear* rather than freeze at a stale
  figure. That is deliberate — `tvl_drain` reads the same view and must not
  compute ratios against a quote from yesterday.
- `pool_price_snapshot` is the one **exempt** view, and the exemption is the
  policy's boundary rather than an oversight: it publishes no USD figure, only
  raw inputs with their `fetched_at`, and `price_oracle_deviation` gates on them
  in Rust alongside its `max_spot_age` guard. **The policy binds valuation, not
  comparison.**

`price_staleness.rs` enumerates the price-reading views from `pg_views` and fails
on any whose **bound count differs from its lookup count** — per lookup, not per
view, so a view with five price LATERALs and one bound is caught. A view that
reads `token_prices` without any countable lookup (a plain `JOIN` deparses
without `FROM token_prices`) is a failure too, not a pass. So a view added later
cannot quietly opt out. It is a tripwire for the forgotten site, not a proof:
substring counting cannot tell two bounds on one lookup from one bound each.

Note what this does *not* cover: a mint carries no price at all before
`yog-context` first knows it. That is **absence**, not expiry, and no staleness
bound reaches it — see the implied price below.

### Three ways to say "we don't know", and what each one does

A missing price does not behave the same way everywhere in the chain:

| mechanism | effect | where |
|---|---|---|
| `INNER JOIN token_metadata` | the row **disappears** | top of §15's views — unresolved mints |
| `LEFT JOIN LATERAL` on the price | the value is **NULL**, and NULL propagates through the whole arithmetic expression | the valuation CTEs |
| ~~`COALESCE(…, 0)` downstream~~ | ~~NULL becomes a **hard zero**~~ | **removed in migration 006** — see below |

The third mechanism is gone from the signal-engine flow repositories, and it is
not coming back: coalescing the two swap directions *independently* turned one
missing price into `(0 − X)/X = −1.0` exactly, a guaranteed maximum-magnitude
`flow_imbalance` Critical on a possibly balanced pool. The rule is now:

> **A flow whose window is not entirely valuable is UNKNOWN, not zero.**

Expressed identically at both call sites, over a `valuation_complete` column the
two flow views carry:

```sql
CASE WHEN bool_and(valuation_complete) THEN SUM(<column>) END
```

⚠️ **`bool_and` is not decoration.** Dropping the `COALESCE` alone is *not
enough*: `SUM` skips NULLs by itself, so a window where only some hours are
valuable returns a **sub-total with no NULL anywhere** — the silent half of the
defect, and the one that costs a missed signal rather than a false one.
Requiring the whole window makes the sub-total unrepresentable.

`SUM` skipping NULLs is also why the analytics repositories ship **coverage
counters**
next to the sums (`swap_buckets_priced_24h` / `swap_buckets_24h`, mirroring
`pools_priced` / `pools_observed`): the value alone cannot say how complete it
is. Adding a new aggregate over a valued view means adding its coverage too.

⚠️ **A coverage denominator must be counted where the rows still exist.** The
first mechanism above deletes rows, so counting over a view that INNER-joins
`token_metadata` counts only the buckets that *survived* — and reports 100 %
over a window whose unresolved-mint pools were dropped whole. That is why
`meteora_damm_v2_swap_events_hourly_priced` LEFT-joins metadata: the bucket
stays, unvaluable, and lands in the denominator instead of vanishing from both
sides of the ratio.

### The implied price (migration 002)

For **swaps only**, when one of the two tokens has no observed price, the
bucket is valued through the exchange rate its own swaps traded at, anchored on
the other token's observed price — exposed per (pool, hour) by
`meteora_damm_v2_swap_events_hourly_priced`, with `price_a_implied` /
`price_b_implied` saying when it was used.

The rule and its boundary:

- it is a **measurement**, not an extrapolation — the rate comes from trades
  inside that hour, and anchoring on the hard asset (SOL, USDC) is also the
  more robust choice;
- ⚠️ **it is net of the trading fee, and the bound is unknown.** `amount_in`
  includes the fee and `amount_out` does not, so a one-directional hour yields
  `implied = true × (1 − f)` — i.e. `volume_usd` measures the **output** leg,
  where a both-priced bucket measures the **input** leg. Two conventions in one
  column, selected by whether a price was observed. The algebra cancels when
  the hour's flow is balanced, but that case is rare in practice (**35 of 36
  implied buckets were one-directional** on 5 August 2026 — a token thin enough
  to need the fallback trades one way at a time). **`f` is now readable**, which
  it was not when 002 shipped: `pools.fee_bps` is still the genesis cliff — off
  by ×5 and ×49 where it was checked — but migration 004 stores the decay curve
  and `amm::damm_v2::base_fee_numerator_at` evaluates it, so the fee actually in
  force is available to bound this error. ⚠️ Available, **not yet applied**: no
  consumer of `volume_usd` uses it today, and bounding the published figure with
  it is its own change. Two gaps remain regardless — a market-cap scheduler or a
  rate limiter has no time curve to evaluate, and a slot-activated pool is not
  evaluated at all (none has been observed). Still a large net gain over a NULL,
  which carries no information at all. `price_a_implied` / `price_b_implied` say
  which buckets used the fallback, but they stop at the view: propagating them
  to the API and the dashboard is a product decision that has not been made, so
  do not assume a consumer knows;
- **neither side priced → still NULL.** No fallback onto a later price exists,
  by decision: if we don't know, we don't know;
- it is **not** applied to liquidity or claim amounts. A liquidity add has no
  counter-leg anchoring it, so a trade rate there would be an extrapolation. A
  rate is only used on the flow that produced it.

**Valuability is decided per bucket, not per figure** — `valuation_complete` on
the same view. Volume and the fee figures draw on different amounts, so deciding
figure by figure let them stop being NULL together, and three consumers assume
they are: the fee split (whose LP share is a subtraction, and would go negative),
`effectiveFeeBps` (which then divides two disjoint sets of hours), and the
coverage counters (keyed on `volume_usd` alone). A side is *required*
when it carries any amount at all, and a required side needs **both** a price
and a scale — an observed price without a `token_metadata` row still yields
`POWER(10, NULL)`. A side carrying nothing is not required, which is what lets a
one-way hour be valued from the side that actually traded.

### The realized fee split (migration 007)

`meteora_damm_v2_pool_hourly_activity` publishes the realized trading fee as a
total plus **three shares that partition it exactly**:

```
fees_usd = lp_fees_usd + protocol_fees_usd + referral_fees_usd
```

They come from the cascade cp-amm applies in `state/fee.rs::split_fees`. The
part worth memorising is that the **referral is taken out of the protocol
share**, not the LP one:

```
protocol_fee_brut = fee_amount × protocol_fee_percent / 100
trading_fee       = fee_amount − protocol_fee_brut   → claiming + compounding
referral_fee      = protocol_fee_brut × referral_fee_percent / 100
protocol_fee      = protocol_fee_brut − referral_fee   ← what the event carries
```

So the LP share is `claiming + compounding`, and ⚠️ **`fees − protocol` is
wrong** — it credits the referral to the liquidity providers. That was the
published figure until migration 007 (`.project` ticket 05), measured at 0,14 %
to 0,89 % of a pool's fees. The four components summing to the total was never
in question: the emitted `protocol_fee` is already net, so `fee_in_*` in the
cagg double-counts nothing. Only the split was wrong.

The split is computed **once, in SQL**, and read straight through `core`,
the DTOs and the dashboard. It was previously derived in two presentation sites
with a copy of the formula each. Do not re-derive one share from the others —
that is how the defect returns.

`fee_in_a` / `fee_in_b` in the cagg deliberately stay the total of the four
components: that is Meteora's "Trading Fee", the denominator of
`effectiveFeeBps`, and what keeps the three shares additive.

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
(`040`). Post-baseline: `002` rebuilds the swap cagg with the two whole-bucket
traded totals and adds `meteora_damm_v2_swap_events_hourly_priced` — the
implied-price rule above.

### A refresh policy must stay inside its retention policy

The four hourly continuous aggregates each sit on a raw hypertable with a
30-day retention. The relation between the two policies is a **rule, not a
preference**:

```
start_offset  <  drop_after
```

`drop_chunks` logs an invalidation over the range it removes, and a refresh is
invalidation-driven — so a refresh window that reaches past the retention
recomputes a range whose raw rows are gone and writes back nothing, **deleting**
the materialized buckets. The retention never touches the aggregate; the refresh
does. Migration `008` moved the four policies to `29 days` for that reason; the
measurement, and why the 7-day chunk geometry does not save you, are in
[`migrations/README.md`](./migrations/README.md).

☠️ **The rule does not close the hole, it only closes the policy.** A refresh
someone types is not constrained by it: on a database where retention has
already dropped chunks, `refresh_continuous_aggregate(cagg, NULL, NULL)`
processes every accumulated invalidation at once and deletes the history —
measured at **2160 buckets → 779** on a database with 008 applied. Before
retention has ever dropped a chunk the same command is harmless and is the right
way to capture a backlog. Both sides, and how to tell which one you are on, are
in [`migrations/README.md`](./migrations/README.md).

Three integration tests hold the line (`tests/cagg_retention.rs`): one reads the
pairs out of the TimescaleDB catalog and asserts the inequality, one reproduces
the destruction the inequality prevents, and one pins the destruction that
survives it — asserting a loss, so the warning above stays falsifiable. Adding a
fifth aggregate means adding its refresh policy; the first test counts the
aggregates separately from the join, so one that never declares a policy fails
loudly instead of silently opting out. A source hypertable with **no** retention
policy is fine and exempt (`signals` is the precedent): nothing clears its rows,
so no offset can reach a cleared range.

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

Provisioning script applied as the admin role, by
`yog-migrate -- setup-roles`. Creates the five runtime roles, transfers `public`
schema ownership to `yog_migrate`, and sets `ALTER DEFAULT PRIVILEGES FOR ROLE
yog_migrate` so tables created by future migrations inherit the right `SELECT`
grants automatically. It contains no table-specific GRANTs — those live in the
migrations. The role → rights → process mapping is documented in
[`crates/README.md`](../README.md#database-roles).

**Two scopes, one file, and it is idempotent.** Roles are cluster-wide;
everything else is per-database. That asymmetry used to make the script
un-rerunnable — bootstrapping a second database of the same cluster aborted on
`role "yog_migrate" already exists` *before* reaching the per-database half that
was the point of running it. The `CREATE ROLE`s now sit behind a guarded `DO`
block, so re-running is a no-op and a new database of an existing cluster gets
its privileges without touching the roles.

The guard also means a rerun **never resets an existing role's password** — it
must not silently push a production credential back to `CHANGE_ME_…`. Verified
by comparing `pg_authid.rolpassword` before and after a rerun, with a mutation
control proving the comparison distinguishes two hashes.

⚠️ *Not* by logging in, and the reason is a trap worth knowing: the compose
`pg_hba.conf` is `trust` for `local` and `127.0.0.1/32`, `scram-sha-256` for
everything else. A check run through `docker exec … psql` therefore matches the
trust lines and succeeds **with any password, including a deliberately wrong
one** — while the same check from the host over `:5433` authenticates normally.
The first version of this verification connected from inside the container and
was green in both directions; only the negative control exposed it.

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
cargo sqlx prepare -- --all-targets --all-features
```

⚠️ **The trailing flags are not optional**, and this file used to omit them. A
bare `cargo sqlx prepare` compiles only the lib and bins, never sees the
`query!` calls inside `tests/`, and rather than leaving their cache entries
alone it **deletes** them. `sqlx-check` does not catch it either — it runs
without `--all-features`, so those queries are never expanded, and it tolerates
extra entries. The breakage surfaces later as an offline compile error in the
`test-integration` job, far from its cause. Same warning, with the measurement,
in `CLAUDE.md`.

## Integration tests

DB-backed tests live in `tests/` and are gated on the `integration-tests`
feature — `tests/main.rs` opens with `#![cfg(feature = "integration-tests")]`:

```bash
cargo test -p yog-persistence --features integration-tests
```

⚠️ They are **not** `#[ignore]`d, whatever this file used to say. The runs
report `0 ignored` and `--include-ignored` is a no-op here; without the feature
the whole target compiles to nothing rather than to a list of skipped tests.

They need a live Postgres running with `timescaledb.max_background_workers = 0`
(as configured in `docker-compose.yml`): `sqlx::test` creates a fresh database
per test, and the cagg refresh policies of the baseline (§13) otherwise have
the TimescaleDB job scheduler race the next test's migration DDL on the shared
catalog ("tuple concurrently deleted"). Turning the scheduler on deliberately,
to watch the policies run, has its own recipe in
[`migrations/README.md`](./migrations/README.md).

`DATABASE_URL` must point at the **admin** role (`yog`), not `yog_migrate`:
`sqlx::test` builds a throwaway schema in the maintenance database and
`yog_migrate` lacks CREATE on it. The symptom is every test failing in about a
second on SQLSTATE 42501, which reads like a regression and is not one.

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
| `protocol` | `TEXT NOT NULL` | Protocol identifier (`meteora_damm_v2`, etc.) |
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

> ⚠️ **Do not reseed from the table below.** It is a *record* of an April 2026
> calibration window, not a shopping list. Every pool in it is described as a
> burst that went quiet — reseeding them subscribes the indexer to dead
> addresses, and it will sit there receiving nothing while looking perfectly
> healthy. Learned the hard way on 4 August 2026, after a database recreate.
>
> To seed a working allowlist, pick pools that are trading **now** — see
> *Choosing pools to watch* below.

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

### Choosing pools to watch

The allowlist decides what the indexer subscribes to, so a seed made of quiet
pools produces a daemon that runs, logs nothing alarming, and collects nothing.
Pick on **recent** activity, not on a historical table.

Meteora's public API ranks them, and its `volume` object is nested — the
30-minute bucket is the one that says "trading right now", where `24h` can be
a burst that ended this morning:

```bash
curl -s "https://damm-v2.datapi.meteora.ag/pools?limit=60&order_by=volume24h&order=desc" \
  | python3 -c "
import json, sys
rows = [(p['address'], p.get('name','?'), p['volume'].get('30m',0), p.get('tvl',0))
        for p in json.load(sys.stdin)['data']]
for a, n, v30, tvl in sorted(rows, key=lambda r: -r[2])[:10]:
    print(f'{a:<45}{n:<18}{v30:>12,.0f}{tvl:>12,.0f}')
"
```

Worth including deliberately, rather than just taking the top of the list:

- **a pool that is central to routing** (SOL-USDC is the universal
  intermediary). Routed transactions are what exercise `event_index`, the
  multi-hop path, and the same-slot ambiguity counter — none of which a set of
  isolated pools will ever trigger;
- **one deep-TVL pool with a USDC quote** and **one thin-TVL, high-turnover
  pool** — the valuation paths behave differently, and a seed made only of
  memecoin pairs leaves half the read paths untested.

### Seeding the allowlist

The seed lives in [`setup_watched_pools.sql`](src/bin/scripts/setup_watched_pools.sql) and is
applied by the migrate binary:

```bash
cargo run -p yog-persistence --bin yog-migrate -- seed-watched-pools
```

It runs under `DATABASE_URL_ADMIN`: the allowlist is *configuration*, not
runtime data, and the convention keeps configuration writes under the admin
role. `ON CONFLICT DO NOTHING` makes it idempotent, and it never deactivates or
removes a row — curating the allowlist stays manual (see *Administration
helpers* below).

Without at least one active row, an indexer started in pool-centric mode has
nothing to subscribe to and exits on `NoSubscriptionTargets` — this step is not
optional.

**The file seeds exactly one pool, and that is deliberate.** SOL-USDC, the
universal routing intermediary: the only pick whose justification does not
decay, and the single most valuable pool to observe, because routed
transactions are what exercise `event_index`, the multi-hop path and the
same-slot ambiguity counter. A committed top-ten would recreate the 4 August
failure described above — hot today, dead next week, and silent about it.

Everything else is picked fresh at seeding time, into the commented block the
file ends with. *Choosing pools to watch* above is the method.

### Administration helpers

These are the four operations you'll run by hand to manage the allowlist
ad-hoc. They are intended for the admin role:

```sql
-- Add a pool
INSERT INTO watched_pools (pool_address, protocol, note)
VALUES ('<pubkey>', 'meteora_damm_v2', 'manual selection: high TVL');

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
