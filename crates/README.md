# crates/

This directory hosts the Rust workspace — the engine of yog-sothoth.

The workspace follows a **Domain-Driven Design** layout: domain types and contracts live in `core`, infrastructure and I/O live in dedicated adapter crates (`persistence` for Postgres, `bootstrap` for startup utilities). The three native binaries (`indexer`, `api`, `context`) are thin assembly layers that wire the pieces together; a one-shot binary (`yog-migrate`) lives next to the migrations it applies.

This README covers the inter-crate architecture, the responsibilities of each crate, and the conventions a contributor needs to know. For the project-wide pitch, the high-level diagram, and the roadmap, see the [root README](../README.md).

---

## Conventions

The same principles guide every crate. They are not aspirational — the code is structured this way today, and a PR that breaks them is unlikely to be accepted.

- **Single responsibility per layer.** `core` knows no I/O. `persistence` knows no business logic. Binaries do no business logic and no SQL — they wire repositories into the runtime and route between them.
- **Repository traits in `core`, implementations in `persistence`.** The trait declares the contract; the implementation provides the SQL. Binaries depend on the trait, never on the concrete type.
- **Typed errors at every layer boundary.** `RepositoryError` at the persistence boundary, `ApiError` at the HTTP boundary, typed pipeline errors at each indexer stage. A `?` operator that crosses a boundary maps the error explicitly.
- **Skip-and-log over abort-and-die.** Partial failures (a malformed event, a failed insert) are logged, counted, and stepped over. Loop-level failures (closed channel, exhausted semaphore, panic) bubble up and trigger a clean shutdown.
- **Domain types are infra-neutral.** Addresses are `Pubkey`. Decimal prices are `rust_decimal::Decimal`. Lossless `u128` values are `BigDecimal` only at the persistence boundary (`NUMERIC(39, 0)` in Postgres). No `sqlx::types` leaks into `core`.
- **Per-protocol typing all the way down.** Domain events, SQL tables, repositories and sub-persistors are all scoped per `(platform, protocol)` pair — `MeteoraDammV2SwapEvent`, `meteora_damm_v2_swap_events`, `PgMeteoraDammV2SwapEventRepository`. The `DomainEvent` enum is two-level: outer variant per protocol, inner sub-enum per event kind. New protocols add a new outer variant without polluting the existing ones.

---

## Structure

```
crates/
├── core/          ← shared library: domain types, AMM math, protocol extraction
├── persistence/   ← Postgres adapter: repository impls, migrations, yog-migrate binary
├── bootstrap/     ← shared startup utilities: env helpers, SecretUrl, init_rustls/tracing
├── indexer/       ← native binary: Solana RPC ingestion → DB
├── api/           ← native binary: axum HTTP server over the indexed data
├── context/       ← native binary: token enrichment (Helius DAS + Jupiter Price V3)
└── wasm/          ← WASM build target (scaffold — deferred to v0.3)
```

The dependency graph is strict and one-directional:

```
                       ┌──────────┐
                       │   core   │  no I/O, wasm-compatible
                       └────▲─────┘
                            │
              ┌─────────────┼─────────────┬─────────┐
              │             │             │         │
        ┌─────┴─────┐ ┌─────┴─────┐  ┌────┴────┐    │
        │persistence│ │ bootstrap │  │  wasm   │    │
        └─────▲─────┘ └─────▲─────┘  └─────────┘    │
              │             │                       │
              └──────┬──────┘                       │
                     │                              │
        ┌────────────┼────────────┐                 │
        │            │            │                 │
   ┌────┴────┐  ┌────┴────┐  ┌────┴─────┐           │
   │ indexer │  │   api   │  │ context  │           │
   └─────────┘  └─────────┘  └──────────┘           │
                                                    │
                                          (no binary depends on wasm)
```

`core` knows nothing about Postgres, axum, HTTP clients, or even the standard library's environment. It declares traits; the adapters and binaries implement and consume them. Each binary depends only on `core` (for types), `persistence` (when it needs the DB), and `bootstrap` (for startup helpers).

---

## `core` (`yog-core`)

Pure logic and domain types. No I/O, no runtime, no database.

### Layout

```
core/src/
├── domain/                                  ← entities + repository contracts
│   ├── meteora/                             (Meteora-family domain events)
│   │   ├── damm_v2/                         (one module per event kind — 11 today)
│   │   │   ├── swap_event/                  (MeteoraDammV2SwapEvent + repo trait)
│   │   │   ├── liquidity_event/             (MeteoraDammV2LiquidityEvent + repo)
│   │   │   ├── claim_position_fee_event/    (MeteoraDammV2ClaimPositionFeeEvent + repo)
│   │   │   ├── claim_reward_event/          (MeteoraDammV2ClaimRewardEvent + repo)
│   │   │   ├── create_position_event/       (MeteoraDammV2CreatePositionEvent + repo)
│   │   │   ├── close_position_event/        (MeteoraDammV2ClosePositionEvent + repo)
│   │   │   ├── lock_position_event/         (MeteoraDammV2LockPositionEvent + repo)
│   │   │   ├── permanent_lock_position_event/ (MeteoraDammV2PermanentLockPositionEvent + repo)
│   │   │   ├── initialize_pool_event/       (MeteoraDammV2InitializePoolEvent + repo)
│   │   │   ├── set_pool_status_event/       (MeteoraDammV2SetPoolStatusEvent + repo)
│   │   │   ├── update_pool_fees_event/      (MeteoraDammV2UpdatePoolFeesEvent + repo)
│   │   │   └── damm_v2.rs                   (MeteoraDammV2Event sub-enum)
│   │   └── meteora.rs                       (mod file)
│   ├── pool/                                (Pool, PoolRepository — cross-protocol)
│   ├── pool_current_state/                  (CQRS projection — cross-protocol)
│   ├── pool_analytics/                      (hourly aggregates — cross-protocol)
│   ├── token_metadata/                      (TokenMetadata + repo)
│   ├── token_price/                         (TokenPrice + repo, PriceProvider)
│   ├── network_status/                      (singleton snapshot)
│   ├── watched_pool/                        (allowlist)
│   ├── freshness_status/
│   ├── protocol/                            (Protocol enum)
│   ├── trade_direction.rs
│   └── domain_event.rs                      (two-level DomainEvent enum)
├── application/
│   └── extraction/                          ← transaction → domain events use case
│       ├── meteora/damm_v2/                 (active — v0.1)
│       │   ├── events.rs                    (wire events, borsh mirrors)
│       │   ├── extractor.rs                 (Anchor event_cpi extraction)
│       │   └── translator.rs                (wire → domain)
│       ├── anchor_event.rs                  (generic Anchor event_cpi decoder)
│       ├── event_extractor.rs               (EventExtractor trait)
│       ├── extraction_dispatcher.rs         (ExtractionDispatcher struct)
│       ├── meteora.rs
│       └── outcome.rs                       (ExtractionOutcome, ExtractionFailure)
├── amm/                                     ← pure AMM math
├── tools/
│   └── pagination.rs                        (Page<T>, Cursor enum)
├── error/                                   ← CoreError, RepositoryError, CoreResult<T>
└── solana_types.rs                          ← re-export hub for Solana SDK types
```

### Responsibilities

- **Domain models** (`domain/`) — entities and the repository traits that define every persistence contract (`PoolRepository`, `MeteoraDammV2SwapEventRepository`, `TokenMetadataRepository`, …). Per-protocol events live under `domain/<platform>/<product>/`; cross-protocol concepts (`Pool`, `PoolCurrentState`, `TokenPrice`, …) sit at the root of `domain/`.
- **Two-level `DomainEvent`** (`domain/domain_event.rs`) — sum type with one outer variant per protocol, delegating to a sub-enum per event kind. `DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(...))` is the canonical shape. Accessors (`pool_address`, `signature`, `timestamp`, `protocol`, `kind`) delegate to the inner sub-enum, which carries the same accessors against its concrete variants.
- **Event extraction** (`application/extraction/`) — the use case that turns raw Solana transactions into protocol-agnostic `DomainEvent`s. Lives in `application/` rather than `domain/` because it orchestrates an external concern (the Solana transaction shape) into the domain language. Per-protocol implementations of the `EventExtractor` trait (Anchor `event_cpi` decoders + translators) sit under `extraction/<platform>/<product>/`.
- **AMM math** (`amm/`) — formulas for price, reserves, slippage, imbalance. Kept here because they will eventually run in the browser too via WASM (deferred — see [`wasm`](#wasm-yog-wasm)).
- **Pagination** (`tools/pagination.rs`) — `Page<T>` envelope and discriminated `Cursor` enum used by every paginated repository method.
- **Solana SDK indirection** (`solana_types.rs`) — single point of contact for types reshuffled by Solana SDK releases (`EncodedConfirmedTransactionWithStatusMeta`, `UiInstruction`, `option_serializer`). When the SDK restructures, only this file changes.
- **Errors** (`error/`) — `CoreError` for domain-level failures, `RepositoryError` as the boundary type returned by every repository trait. Adapters convert their internal errors (e.g. `sqlx::Error`) into `RepositoryError` at their public surface.

### `EventExtractor` trait and `ExtractionDispatcher`

The extraction layer has two surface types:

```rust
/// Per-protocol entry point. One implementation per supported protocol.
pub trait EventExtractor: Send + Sync {
    fn program_id(&self) -> &str;
    fn extract_events(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome>;
}

/// Dispatcher that holds one pre-instantiated `EventExtractor` per protocol
/// and routes calls based on the `Protocol` enum.
pub struct ExtractionDispatcher {
    damm_v2: MeteoraDammV2,
    // future: damm_v1, dlmm, raydium_clmm, orca_whirlpool, ...
}

impl ExtractionDispatcher {
    pub fn extract(
        &self,
        protocol: Protocol,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome> { /* match on Protocol */ }
}
```

The trait keeps the per-protocol contract explicit and testable. The dispatcher hides the concrete handlers from callers (`yog-indexer` only depends on `ExtractionDispatcher`), and the enum dispatch is cheap — no `dyn` overhead, no allocation per transaction.

### Anchor `event_cpi` extraction pipeline

Each Meteora program emits its events via Anchor's `emit_cpi!` mechanism — a self-CPI to an `event_authority` PDA, with a stable wire format:

```
[8 bytes EVENT_IX_TAG][8 bytes event discriminator][borsh payload]
```

where `EVENT_IX_TAG = sha256("anchor:event")[..8]` is the fixed prefix injected by Anchor.

The pipeline runs in three stages, each in its own module:

```
EncodedConfirmedTransactionWithStatusMeta
        │
        ▼
[application/extraction/anchor_event.rs]   extract_anchor_event_cpis(tx, program_id)
        │           ├─ iterates over inner_instructions
        │           ├─ filters: programId match + EVENT_IX_TAG prefix
        │           └─ returns Vec<Vec<u8>>  (decoded base58 payloads)
        ▼
[extraction/meteora/damm_v2/events.rs]     match discriminator → DammV2WireEvent::{...}
        │           └─ borsh::deserialize the payload
        ▼
[extraction/meteora/damm_v2/translator.rs] translate_wire_event(wire, transfer_checked_group, ...)
        │           ├─ for Swap2 / LiquidityChange: extract mints from surrounding transferChecked
        │           ├─ compute_fee_token_is_a from (collect_fee_mode, trade_direction)
        │           └─ returns DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(...))
        ▼
ExtractionOutcome { events, unknown, failures }
```

Three failure types are distinguished in `ExtractionFailure` and counted as separate metric labels: `AnchorDecode` (prefix or payload-size mismatch), `Borsh` (schema mismatch), `Translation` (missing transferChecked context, invalid enum value).

### Repository traits

Each domain aggregate that needs persistence declares a repository trait in its module — e.g. `domain/pool/repository.rs`:

```rust
#[async_trait]
pub trait PoolRepository: Send + Sync {
    // Write side (used by the indexer)
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()>;
    async fn touch_last_seen(&self, pool_address: &Pubkey) -> RepositoryResult<()>;

    // Read side (used by the api)
    async fn find_by_address(&self, pool_address: &Pubkey) -> RepositoryResult<Option<Pool>>;
    async fn find_paginated(
        &self,
        cursor: Option<PoolCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Pool>>;
}
```

Per-protocol event repositories follow the same pattern but with protocol-prefixed types — e.g. `MeteoraDammV2SwapEventRepository` operates on `MeteoraDammV2SwapEvent` and `MeteoraDammV2SwapCursor`. At runtime, the connected Postgres role determines which methods actually succeed. The `yog_api` role lacks `INSERT/UPDATE` on event tables, so calling `insert` from the api fails with `permission denied` from Postgres itself, by design (see [Database roles](#database-roles)).

### Conventions and invariants

These invariants are documented on the affected types and enforced at construction time:

- **Mints sorted by raw bytes** — in `Pool`, `MeteoraDammV2SwapEvent`, `MeteoraDammV2LiquidityEvent`, `token_a_mint` and `token_b_mint` are ordered by `Pubkey::Ord` (raw bytes). Stable regardless of swap direction. Differs from the Meteora SDK canonical convention; documented on each affected struct.
- **Canonical `(token_a, token_b)` exposure** — DAMM v2 swap and liquidity events expose `amount_a` / `amount_b` and `reserve_a_after` / `reserve_b_after` in canonical order. Swap direction lives in the `TradeDirection` enum (`AtoB` | `BtoA`). Callers reconstruct the trader's perspective by combining the two.
- **No `protocol` field on per-protocol sub-events** — `MeteoraDammV2SwapEvent` and its siblings carry no `protocol: Protocol` field. The protocol identity is encoded by the outer `DomainEvent` variant and by the SQL table name itself; storing it on the inner struct would be redundant.
- **`fee_token_is_a` precomputed** — boolean stored on `MeteoraDammV2SwapEvent`, derived from `(collect_fee_mode, trade_direction)` in the translator. Mirrors `cp-amm::FeeMode::get_fee_mode`. Avoids recomputation at query time.
- **Four fee components separated** — `claiming_fee`, `protocol_fee`, `compounding_fee`, `referral_fee`. Lets v0.2 signal detectors (e.g. fee yield spike) distinguish LP yield from protocol revenue.
- **Lossless `u128` in DB** — `next_sqrt_price` (Q64.64) and `liquidity_delta` are stored as `NUMERIC(39, 0)`. Conversion via dedicated helpers in `persistence::repositories::helper`.
- **Off-chain decimal prices** — `TokenPrice::price_usd` is a `rust_decimal::Decimal` (infra-neutral, no `sqlx` leak), persisted as `NUMERIC(38, 18)`.

### Compilation targets

- `cargo build` → native library, linked into `yog-indexer`, `yog-api`, `yog-context` ✅
- `wasm-pack build` → WASM module for the browser 🚧 deferred to **v0.3** (see [`wasm`](#wasm-yog-wasm))

---

## `persistence` (`yog-persistence`)

Postgres adapter. Concrete implementations of the repository traits declared in `core`, the migration suite, and the one-shot `yog-migrate` binary that applies it.

### Layout

```
persistence/
├── migrations/                            ← sqlx migrations (forward-only)
│   ├── 001_initial_schema.sql             (consolidated v0.1 baseline)
│   └── README.md                          (forward-only convention, GRANT policy)
├── setup_roles.sql                        ← one-time role provisioning (admin)
└── src/
    ├── database.rs                        ← Database::connect, run_migrations
    ├── health.rs                          ← PgHealthChecker
    ├── repositories/                      ← one impl per domain repository trait
    │   ├── helper/                        (string→Pubkey, u64↔i64, u128↔BigDecimal,
    │   │                                   pagination helpers, sqlx error mapping)
    │   ├── meteora/                       (per-protocol event repositories — one per event kind)
    │   │   ├── damm_v2/
    │   │   │   ├── swap_event/            (PgMeteoraDammV2SwapEventRepository + Row)
    │   │   │   ├── liquidity_event/
    │   │   │   ├── position_fee_claim/    (PgMeteoraDammV2ClaimPositionFeeEventRepository)
    │   │   │   ├── reward_claim/          (PgMeteoraDammV2ClaimRewardEventRepository)
    │   │   │   ├── create_position.rs
    │   │   │   ├── close_position.rs
    │   │   │   ├── lock_position.rs
    │   │   │   ├── permanent_lock_position.rs
    │   │   │   ├── initialize_pool.rs
    │   │   │   ├── set_pool_status.rs
    │   │   │   └── update_pool_fees.rs
    │   │   └── damm_v2.rs                 (mod file)
    │   ├── pool/                          (PgPoolRepository — cross-protocol)
    │   ├── pool_current_state/            (cross-protocol projection)
    │   ├── pool_analytics/
    │   ├── network_status/
    │   ├── token_metadata/
    │   ├── token_price/
    │   ├── watched_pool/
    │   └── event_freshness.rs
    └── bin/
        └── migrate.rs                     ← yog-migrate binary (~30 lines)
```

### Responsibilities

- **Repository implementations** — one `Pg*Repository` per domain aggregate. Each takes a `PgPool` in its constructor; the pool is owned by the consumer (each binary instantiates its own pool with its own role credentials).
- **Connection management** — `Database::connect(url)` returns a thin wrapper over `sqlx::PgPool`. `Database::run_migrations()` exposes `sqlx::migrate!()` behind a clean signature so the `yog-migrate` binary can call a domain method, not sqlx directly.
- **Conversion helpers** (`repositories/helper`) — `parse_pubkey`, `convert_u64_to_i64`, `convert_bigdecimal_to_u128`, etc. Uniform error mapping via `map_sqlx_error` which translates `sqlx::Error` variants into the right `RepositoryError` semantic (`NotFound`, `Conflict`, `Timeout`, `Backend`, `Integrity`).
- **Schema migrations** (`migrations/`) — sqlx-managed, source of truth at deployment time. Applied by `yog-migrate` (a binary) or `cargo sqlx migrate run` (in CI / locally), both running under the `yog_migrate` DDL role.

### Per-protocol table strategy ("voie 3")

Each `(protocol, event_kind)` combination has its own SQL table, named `<platform>_<product>_<event_kind>_events`. The DAMM v2 family currently ships eleven tables:

```
meteora_damm_v2_swap_events
meteora_damm_v2_liquidity_events
meteora_damm_v2_claim_position_fee_events
meteora_damm_v2_claim_reward_events
meteora_damm_v2_create_position_events
meteora_damm_v2_close_position_events
meteora_damm_v2_lock_position_events
meteora_damm_v2_permanent_lock_position_events
meteora_damm_v2_initialize_pool_events
meteora_damm_v2_set_pool_status_events
meteora_damm_v2_update_pool_fees_events
```

Each table holds only the columns relevant to its protocol — no NULL columns for protocol-incompatible fields, no generic JSONB blob. When DLMM, Raydium CLMM or Orca Whirlpool land, they get their own sibling tables (e.g. `meteora_dlmm_swap_events`) with their own schemas.

For unified reads ("all swaps for this pool, regardless of protocol"), the baseline provides cross-protocol SQL **VIEW**s at the bottom of `001_initial_schema.sql`:

```
swap_events                  (UNION ALL over meteora_damm_v2_swap_events, ...)
liquidity_events
claim_position_fee_events
claim_reward_events
```

Each VIEW exposes the slim common columns plus a synthesised `protocol` column (`'meteora_damm_v2'::TEXT AS protocol`). Protocol-specific columns (`next_sqrt_price`, fee breakdown, etc.) are NOT in the VIEWs — code that needs them reads the underlying table directly. New protocols add a `UNION ALL` branch to each VIEW without touching the API code.

The VIEWs cover only the four concepts that already have a cross-protocol meaning. The newer DAMM v2 tables (position lifecycle — `create_position`, `close_position`, `lock_position`, `permanent_lock_position` — and pool administration — `initialize_pool`, `set_pool_status`, `update_pool_fees`) have no cross-protocol VIEW yet; they are read per-protocol directly. A VIEW is added only once a second protocol exposes the same concept.

The `pools`, `pool_current_state`, `watched_pools`, `network_status`, `token_metadata`, `token_prices` tables stay generic (one table for all protocols), with a `protocol` column where useful.

### The `yog-migrate` binary

`crates/persistence/src/bin/migrate.rs` is a small (~30 LOC) one-shot binary:

```bash
cargo run -p yog-persistence --bin yog-migrate
```

It reads `DATABASE_URL_MIGRATE` from the environment, connects under the `yog_migrate` role, applies any pending migration via `Database::run_migrations()`, and exits 0. In Docker, it runs once at compose-up time; runtime services depend on it via `service_completed_successfully` so they never start against a half-migrated schema.

`yog-migrate` is the **only** path through which DDL flows in production. The four runtime roles cannot CREATE or ALTER tables — by design.

### Migrations conventions

- **Forward-only.** Migrations committed to git never change. No `.down.sql`. Rollback in production is a backup restore.
- **GRANTs live in the migration that creates the table.** Each `CREATE TABLE` is followed by its `GRANT INSERT, UPDATE` (and any other non-default) statements. `SELECT` is covered by the default privileges set in `setup_roles.sql`.

The v0.1 baseline (`001_initial_schema.sql`) consolidates the early dev migrations into a single applicable unit. From this baseline onwards, forward-only resumes — new migrations are added as `00N_*.sql` and never rewritten. The DAMM v2 position-lifecycle and pool-admin tables landed this way (`002_create_position_events.sql` through `008_update_pool_fees_events.sql`), each carrying its own `GRANT ... TO yog_indexer`.

### `setup_roles.sql`

Slim provisioning script applied once per database as superuser. Creates the four runtime roles, transfers `public` schema ownership to `yog_migrate`, and sets `ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate` so the tables created by future migrations inherit the right `SELECT` grants automatically. It contains no table-specific GRANTs.

### Database roles

| Role | Permissions | Used by |
|---|---|---|
| `yog_migrate` | DDL — owns the schema, applies migrations | `yog-migrate` binary, `cargo sqlx migrate run` |
| `yog_indexer` | `SELECT, INSERT, UPDATE` on event tables and pool registry; `SELECT` on `watched_pools` | indexer process |
| `yog_api` | `SELECT` across event tables, VIEWs, and enrichment tables | api process |
| `yog_context` | `SELECT, INSERT, UPDATE` on `token_metadata` and `token_prices`; `SELECT` on `pools` | context process |
| admin (e.g. `yog` superuser) | Full — provisioning, `cargo sqlx prepare`, ad-hoc operations | tooling only, never a running service |

### Pattern for repository implementations

```rust
pub struct PgMeteoraDammV2SwapEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2SwapEventRepository {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
}

#[async_trait]
impl MeteoraDammV2SwapEventRepository for PgMeteoraDammV2SwapEventRepository {
    // sqlx::query! / query_as! against self.pool,
    // errors mapped via map_sqlx_error,
    // row → domain conversion via TryFrom<XxxRow> for XxxEvent
    // living in the sibling rows.rs.
}
```

Row types follow the convention `Row + TryFrom<XxxRow> for XxxDomain`: SQL types in (String, i64, BigDecimal, ...), domain types out (Pubkey, u64, u128, ...). Any parsing failure surfaces as `RepositoryError::Integrity`.

### SQLx offline cache

The crate uses `sqlx::query!` macros that verify SQL syntax against the live schema at compile time. The verified query cache is committed under `crates/persistence/.sqlx/`, which allows the workspace to build everywhere with `SQLX_OFFLINE=true`.

**After modifying any `sqlx::query!` call**, regenerate the cache before committing:

```bash
cd crates/persistence
cargo sqlx prepare
```

CI runs `cargo sqlx prepare --check` against a real Postgres with migrations applied.

---

## `bootstrap` (`yog-bootstrap`)

Shared startup utilities for the native binaries. Hosts what every binary needs at startup, and only that.

### Layout

```
bootstrap/src/
├── env.rs           ← required, parse_required_u32, parse_required_bool
├── secret.rs        ← SecretUrl (redacted Display/Debug)
├── error.rs         ← ConfigError (MissingVariable, InvalidValue)
└── runtime.rs       ← init_rustls, init_tracing
```

### What goes here

Utilities **identical across all native binaries**: env var parsing primitives, the redacting `SecretUrl` wrapper, the canonical `ConfigError`, `init_rustls()` (required by rustls 0.23+ before any TLS handshake), `init_tracing()` (JSON or text output via `LOG_FORMAT`).

### What does NOT go here

Things that vary across binaries stay in their respective binaries: the `Config` struct itself, `init_metrics`, process-specific signal handling, dependency wiring.

The decision rule when adding a new utility: **does this run identically in every binary's `main()`?** If yes, it belongs in `bootstrap`. If it varies even slightly, it stays in the binary.

---

## `indexer` (`yog-indexer`)

Native binary. Long-lived process consuming Solana mainnet WebSocket events and persisting indexed data.

### Layout

```
indexer/src/
├── application/
│   ├── services/
│   │   ├── meteora/damm_v2/
│   │   │   └── event_persistor.rs            (MeteoraDammV2EventPersistor)
│   │   ├── meteora.rs                        (mod file)
│   │   ├── event_persistor.rs                (thin protocol dispatcher)
│   │   ├── event_persistor_metrics.rs        (Prometheus labels)
│   │   ├── indexer_service_metrics.rs        (transaction-processor metrics)
│   │   ├── pool_maintenance.rs               (shared pool & projection helper)
│   │   ├── transaction_processor.rs          (fetch → extract → diagnose → persist)
│   │   └── watched_pool_service.rs           (allowlist management)
│   ├── reporter/
│   │   └── network_status_reporter.rs        (Solana slot/latency snapshot)
│   └── workers/
│       ├── indexer.rs                        (bounded-concurrency consumer)
│       └── subscription.rs                   (WebSocket subscription supervisor)
├── infra/
│   └── rpc/
│       ├── dispatcher/                       (log-event → qualified-signature filtering)
│       ├── types/                            (qualified_signature, raw_log_event)
│       ├── listener.rs                       (WebSocket subscription client)
│       └── transaction_fetcher.rs            (HTTP transaction fetcher + FetchError)
├── bootstrap/
│   ├── config.rs                             (Config::load() — env-driven configuration)
│   └── daemon.rs                             (top-level lifecycle, task wiring, shutdown)
├── error/                                    (typed error per layer)
├── utils/redact.rs                           (API-key scrubbing for logs)
└── main.rs
```

### Three-stage pipeline

The indexer is structured as three Tokio tasks connected by bounded mpsc channels. Each stage has a single responsibility, its own typed error channel, and its own metrics:

```
┌──────────────┐    raw    ┌──────────────────┐  qualified  ┌────────────────┐
│ RpcListener  │──────────▶│ SignatureDispat. │────────────▶│ IndexerWorker  │
│              │  RawLog   │                  │  Signature  │                │
│ logsSubscribe│  Events   │ filter chain:    │  + protocol │ ↓ semaphore-   │
│ + reconnect  │           │ failed / invoc.  │             │   bounded      │
│              │           │ / watched_pool   │             │   spawn        │
└──────────────┘           └──────────────────┘             └────────┬───────┘
                                                                     │
                                                                     ▼
                                                            ┌─────────────────────┐
                                                            │ TransactionProcessor│
                                                            │ fetch (Fetcher) →   │
                                                            │ extract (Dispatcher)│
                                                            │ → persist (Persistor)│
                                                            └─────────────────────┘
```

**`RpcListener`** owns the WebSocket connection, handles reconnection with exponential backoff, and forwards raw log events downstream. It is itself an orchestrator of a fleet of `SubscriptionWorker` instances (one per pool in the allowlist), each with its own retry budget (`RPC_WORKER_MAX_RETRIES`).

**`SignatureDispatcher`** applies a chain of filters that turn raw log events into qualified `(protocol, signature)` pairs — drops failed transactions, transactions that don't actually invoke the watched protocol, and (temporarily) transactions outside the watched-pool allowlist.

**`IndexerWorker`** consumes qualified signatures and drives `TransactionProcessor` with bounded concurrency. The cap is `MAX_CONCURRENT_INDEX_TASKS = 15`, calibrated against the Helius free tier with headroom. Per-signature failures are logged and counted but never stop the pipeline; loop-level failures (closed channels, exhausted semaphore, panics) bubble up to the daemon and trigger graceful shutdown via a shared `CancellationToken`.

### `TransactionProcessor` and its dependencies

`TransactionProcessor::process(protocol, signature)` is the core pipeline. It composes four collaborators, each with one responsibility:

```
TransactionProcessor
├── TransactionFetcher        (infra/rpc/transaction_fetcher.rs)
│   └── async fn fetch(signature) -> Result<Tx, FetchError>
│       (retry loop, encoding config, classified FetchError variants)
├── ExtractionDispatcher      (yog-core, application/extraction/)
│   └── fn extract(protocol, tx) -> CoreResult<ExtractionOutcome>
│       (delegates to MeteoraDammV2, ...)
└── EventPersistor            (application/services/event_persistor.rs)
    └── async fn persist(event: &DomainEvent)
        └── match DomainEvent::MeteoraDammV2(e) → MeteoraDammV2EventPersistor::persist(e)
            ├── match on the sub-enum → persist_swap / persist_liquidity /
            │     persist_claim_* / persist_{create,close,lock,permanent_lock}_position /
            │     persist_{initialize_pool,set_pool_status,update_pool_fees}
            ├── PoolMaintenance (shared) — pool upsert + pool_current_state projection
            └── DammV2Repos — bundle of the per-event-kind XxxEventRepository
                  trait objects (yog-persistence), one Arc each
```

The split is deliberate:

- **`TransactionFetcher`** is domain-agnostic — it knows about RPC and retries, not about Protocol or event kinds. The caller (`TransactionProcessor`) instruments the fetch duration with the right `protocol` label.
- **`ExtractionDispatcher`** lives in `yog-core` and centralises the `Protocol -> handler` mapping. `yog-indexer` no longer imports concrete handlers (`MeteoraDammV2`, …) — adding a new protocol updates `yog-core` only.
- **`EventPersistor`** is a thin dispatcher that matches on the outer `DomainEvent` variant and delegates to a sub-persistor per protocol. The actual persistence recipes live in the sub-persistor (e.g. `MeteoraDammV2EventPersistor`).
- **`PoolMaintenance`** is shared by every sub-persistor via `Arc`. It owns the cross-protocol pool registry (`PoolRepository`) and the per-pool projection (`PoolCurrentStateRepository`). When DLMM lands, it reuses the same instance — no duplication.

### Skip-and-log error semantics

`TransactionProcessor::process` follows a strict skip-and-log policy:

- **Per-event failures don't abort the others** — when `EventPersistor::persist` is called on each extracted event, failures are logged and counted in `persist_failures_total{event_kind}`, and the next event is attempted.
- **Per-signature failures don't stop the worker** — the `IndexerWorker` catches errors from `process`, logs and counts them, and keeps draining the channel.
- **Loop-level failures bubble up** — closed channels, exhausted semaphores, panics in spawned tasks reach `Daemon::run` via typed `IndexerWorkerError` and trigger graceful shutdown of all tasks via the shared `CancellationToken`.

An `ExitGuard` RAII helper ensures every entry into `process` produces an exit counter and duration sample — the guard is constructed at the top of the function, mutated with `guard.set(outcome)` at each exit point, and its `Drop` records the metrics. Covers every early return, including `?`-propagated errors.

### Observability

The indexer exposes Prometheus metrics on `:9000/metrics`. Key families:

- **Pipeline counters** — `raw_log_events_total`, `raw_log_events_rejected_total{filter, reason}`, `qualified_signatures_total`, `downstream_saturated_total`
- **Processor counters** — `index_transaction_entered/exited_total{outcome}`, `transactions_no_match_total`, `unknown_event_total{discriminator}`, `extraction_failure_total{kind}`, `fetch_failures_total{reason}`, `fetch_not_found_total`
- **Persistor counters** — `instructions_indexed_total{protocol, instruction}`, `persist_failures_total{protocol, event_kind}` — labelled with both protocol and event kind to slice the per-protocol error rates
- **Allowlist filter** — `watched_pool_filter_passed_total{pool_address}`, `watched_pool_filter_dropped_total`
- **Histograms** — `fetch_duration_seconds`, `persist_duration_seconds{protocol, kind}`, `index_transaction_duration_seconds{outcome}`
- **Gauges** — `watched_pools_active`

### Configuration

```env
DATABASE_URL_INDEXER=postgresql://yog_indexer:...@host:5433/yog_sothoth
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
SOLANA_RPC_HTTP=https://api.mainnet-beta.solana.com
RPC_WORKER_MAX_RETRIES=10
MODE_PROTOCOL_CENTRIC=true
```

Reaches Postgres as `yog_indexer` (RW on event tables, RO on `watched_pools`).

### Run

```bash
cargo run -p yog-indexer
```

---

## `api` (`yog-api`)

Native binary. HTTP server built on axum 0.8 — exposes JSON endpoints over the indexed and enriched data.

### Layout

```
api/src/
├── bootstrap/
│   ├── app_state.rs                                  (AppState — dependency container)
│   └── config.rs                                     (Config::load() — env-driven)
├── application/
│   └── services/
│       ├── meteora_damm_v2_swap_service.rs           (DAMM v2 swap listing)
│       └── meteora_damm_v2_liquidity_service.rs      (DAMM v2 liquidity listing)
├── http/
│   ├── dto/
│   │   ├── request/                                  (request DTOs)
│   │   └── response/                                 (response DTOs)
│   ├── handlers/
│   │   ├── health.rs                                 (/healthz)
│   │   ├── pools.rs                                  (/api/pools, /api/pools/{addr}/...)
│   │   ├── tokens.rs                                 (/api/tokens/{mint})
│   │   └── network_status.rs                         (/api/network/status)
│   ├── cursor.rs                                     (base64/JSON cursor codec)
│   ├── middleware.rs                                 (CORS, security headers)
│   └── error.rs                                      (ApiError, IntoResponse)
└── main.rs
```

### Responsibilities

- **HTTP routing and serving** — builds the axum `Router`, applies the middleware stack, runs the serve loop on the address from `Config::bind_addr`.
- **Dependency container** (`AppState`) — holds shared dependencies as `Arc<dyn Trait>` references. `Clone` is cheap (everything is `Arc`-wrapped).
- **Handlers** — one module per route family. Pure async functions taking axum extractors (`State<AppState>`, `Query<T>`, `Path<T>`) and returning `Result<Json<T>, ApiError>`.
- **Application services** — protocol-specific services that compose repository reads with cursor encoding and response DTO mapping. `MeteoraDammV2SwapService` lives in `meteora_damm_v2_swap_service.rs` and consumes `MeteoraDammV2SwapEventRepository`; when DLMM arrives, a sibling `MeteoraDlmmSwapService` is added next to it.
- **Response DTOs** — wire shapes decoupled from the domain. Public URLs remain protocol-agnostic (e.g. `/api/pools/{addr}/swaps`); the service backend resolves the pool's protocol and reads the matching table. DTOs (`SwapEventResponse`, `LiquidityEventResponse`) currently carry DAMM v2-specific fields directly; when a second protocol lands, they may evolve into serde-tagged enums for a natural discriminated union on the frontend side.
- **Error type** — `ApiError` with `BadRequest`, `NotFound`, `Internal` variants plus an `IntoResponse` impl. Errors follow RFC 9457 Problem Details format (see below).
- **Middleware** — CORS (permissive in dev), security headers.

### Current endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness probe (200 OK, no DB roundtrip) |
| `GET` | `/api/network/status` | Latest indexer/RPC slot, RPC latency, observed timestamp |
| `GET` | `/api/pools` | Paginated list of discovered pools (cursor-based, `limit` 1–200, default 50) |
| `GET` | `/api/pools/{addr}/swaps` | Paginated list of swap events for a pool |
| `GET` | `/api/pools/{addr}/liquidity-events` | Paginated list of liquidity events for a pool |
| `GET` | `/api/tokens/{mint}` | Single token (metadata + latest price). 404 if metadata unknown; 200 with `price: null` if metadata exists but no price yet |

### Error responses

Errors use [RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457) format.

**Content-Type**: `application/problem+json`.

**Wire shape**:

```json
{
  "type": "about:blank",
  "title": "Bad Request",
  "status": 400,
  "detail": "invalid pool address: foo"
}
```

| Status | `title`                 | Common causes |
|--------|-------------------------|---------------|
| 400    | `Bad Request`           | Invalid pool address, malformed cursor, limit out of range, mutually exclusive query params |
| 404    | `Not Found`             | Pool or token unknown, no observed state yet for a known pool |
| 500    | `Internal Server Error` | Database failure, encoding bug. The `detail` is always the generic message `"internal server error"`; the real cause is logged server-side under a `request_id` correlatable via the `x-request-id` response header |

Internal errors are logged with full context but never expose implementation details to the client.

### Cursor wire format

Pagination cursors are **opaque to clients**: base64(url-safe, no-pad) encoding of a JSON-serialized `*CursorWire` struct. Clients pass back the `next_cursor` from the previous response without interpreting it.

### Configuration

```env
DATABASE_URL_API=postgresql://yog_api:...@host:5433/yog_sothoth
API_BIND_ADDR=127.0.0.1:5000
```

### Run

```bash
cargo run -p yog-api
```

Connects to Postgres as `yog_api` (RO across the board).

---

## `context` (`yog-context`)

Native binary. Token enrichment daemon — fills in symbol / name / decimals / logo for every mint observed by the indexer, and refreshes USD prices on a regular cadence.

### Layout

```
context/src/
├── application/
│   ├── source/                                       ← ports (MetadataSource, PriceSource)
│   ├── providers/                                    ← adapters (HeliusDasClient, JupiterPriceClient)
│   ├── workers/                                      ← use cases (MetadataWorker, PriceWorker)
│   ├── bootstrap/                                    ← Daemon::new — composition root
│   └── error/                                        ← SourceError, WorkerError
└── main.rs
```

### Two workers, two cadences

The daemon spawns two independent worker loops:

- **Metadata worker** — every `CONTEXT_METADATA_POLL_SECS` (default 10s), queries `TokenMetadataRepository::list_missing_mints` for mints present in `pools` but absent from `token_metadata`. The `MetadataSource` (Helius DAS) chunks and fetches internally; the worker calls a single `fetch_metadata` and upserts what came back.
- **Price worker** — every `CONTEXT_PRICE_INTERVAL_SECS` (default 30s), queries `TokenMetadataRepository::list_known_mints` and asks Jupiter Price V3 for current USD prices. Same pattern: the `PriceSource` chunks internally; the worker calls `fetch_prices` once and inserts what came back, sharing a single `fetched_at` per tick.

### Resilience contract

Both workers are **deliberately resilient**: HTTP errors, decode errors, and per-row persistence errors are absorbed inside the loop (logged and counted, then `continue`). `Err` returned from a source trait is reserved for structural misconfiguration, not for partial fetch failures — those are handled internally as skip-and-log per chunk.

### Configuration

```env
DATABASE_URL_CONTEXT=postgresql://yog_context:...@host:5433/yog_sothoth
SOLANA_RPC_HTTP=https://mainnet.helius-rpc.com/?api-key=...
JUPITER_URL=https://api.jup.ag/price/v3
JUPITER_API_KEY=...
CONTEXT_METADATA_POLL_SECS=10
CONTEXT_PRICE_INTERVAL_SECS=30
```

Connects to Postgres as `yog_context` (RW on `token_metadata` and `token_prices`, RO on `pools`).

### Run

```bash
cargo run -p yog-context
```

---

## `wasm` (`yog-wasm`)

WebAssembly target for the browser. **Currently a scaffold** — the default `cargo new --lib` template, not yet wired to `yog-core`.

Making it functional requires activating a `wasm` feature on `yog-core`, conditional compilation on Solana-only modules, and abstracting `Pubkey` behind a neutral type alias so the `domain/` layer compiles on both targets. Deferred to **v0.3**.

---

## Local development

Two workflows are supported.

### A. Docker (default, easiest)

```bash
# Postgres only — when running native cargo run alongside
docker compose up -d

# Full backend stack (postgres + migrate + indexer + api + context)
docker compose --profile backend up -d --build

# Everything including the Next.js dashboard
docker compose --profile full up -d --build

# Tail a service's logs
docker compose logs -f yog-indexer

# Tear down with volume removal (full reset)
docker compose --profile full down -v
```

### B. Native `cargo run` (faster inner loop)

```bash
# 1. Start Postgres only
docker compose up -d

# 2. Provision the roles (one-time, as superuser)
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f crates/persistence/setup_roles.sql

# 3. Apply migrations (as yog_migrate)
cargo run -p yog-persistence --bin yog-migrate

# 4. Seed the watched-pools allowlist (one-time)
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f scripts/seed_watched_pools.sql

# 5. Run the binary you're working on
cargo run -p yog-indexer    # or yog-api, or yog-context

# Hit the api
curl http://127.0.0.1:5000/healthz
curl http://127.0.0.1:5000/api/pools | jq
```

### Building, testing, linting

```bash
cargo build
cargo test --workspace --all-features
cargo clippy -p yog-api -p yog-core -p yog-context -p yog-indexer -p yog-persistence \
    --all-targets --all-features -- -D warnings
cargo fmt --all
```

The Rust version is pinned in `rust-toolchain.toml` at the repo root.

---

## CI

GitHub Actions runs on every push and PR to `main`:

- **`crates.yml`** — Rust workspace: `check`, `fmt`, `clippy -D warnings`, `test`, `audit`, `sqlx-check` (spins up TimescaleDB, applies migrations, verifies `.sqlx/` cache)
- **`web-quality.yml`** — Next.js typecheck, lint, vitest

---

## Adding a new protocol

The "voie 3" per-protocol shape means a new protocol creates new domain types, new SQL tables, new repositories, and a new sub-persistor — but each step follows a clean pattern, and the dispatch points stay narrow.

### 1. In `core`

**Extraction side**:

- Create a module under `application/extraction/<platform>/<product>/` (e.g. `extraction/meteora/dlmm/`). Split responsibilities following the DAMM v2 pattern: `events.rs` for wire events (borsh mirrors), `extractor.rs` for walking inner instructions, `translator.rs` for wire → domain translation.
- Create a top-level struct (e.g. `MeteoraDlmm`) and implement `EventExtractor`.
- Add a new branch to `ExtractionDispatcher::extract` that routes the new `Protocol` variant to the new struct.

**Domain side**:

- Create per-event modules under `domain/<platform>/<product>/{event_kind}/` with `model.rs` and `repository.rs`. Event structs are prefixed with the protocol (e.g. `MeteoraDlmmSwapEvent`), as are their cursor types (`MeteoraDlmmSwapCursor`).
- Add the sub-enum `<Platform><Product>Event` in `domain/<platform>/<product>.rs` with one variant per event kind.
- Add an outer variant in `DomainEvent` (`domain/domain_event.rs`) and update the accessor methods (`pool_address`, `signature`, `timestamp`, `protocol`, `kind`) to match.

### 2. In `persistence`

- Add a migration that creates the per-protocol tables (`<platform>_<product>_<event_kind>_events`). Each table holds only the columns relevant to the protocol — no NULL columns for protocol-incompatible fields. Include `GRANT INSERT, UPDATE ON <new_table> TO yog_indexer;`.
- Extend the cross-protocol VIEWs at the bottom of the migration (or the latest one redefining them) with a new `UNION ALL` branch per VIEW. The new branch selects from the new table with the `protocol` literal injected.
- Implement the new `Pg<Platform><Product><EventKind>EventRepository` traits in `persistence/src/repositories/<platform>/<product>/<event_kind>/`. Follow the existing `Row + TryFrom<XxxRow> for XxxDomain` convention.
- Regenerate `.sqlx/` (`cd crates/persistence && cargo sqlx prepare`).
- Re-export the new repositories from `lib.rs`.

### 3. In `indexer`

- Create a sub-persistor `<Platform><Product>EventPersistor` under `application/services/<platform>/<product>/event_persistor.rs`. It owns the per-protocol repos plus an `Arc<PoolMaintenance>`. Its `persist` method matches on the protocol's sub-enum and dispatches to per-variant `persist_<kind>` methods.
- Add a new branch in `EventPersistor::persist` that delegates `DomainEvent::<NewProtocol>(e)` to the new sub-persistor.
- In `Daemon::new` (`bootstrap/daemon.rs`), instantiate the new sub-persistor with its repos plus the shared `PoolMaintenance`, and wire it into the top-level `EventPersistor`.

### 4. In `api` (when read access is needed)

- If the protocol introduces new event kinds the API wants to expose, add a service under `application/services/<platform>_<product>_<event_kind>_service.rs`.
- Add handlers and DTOs as needed. If you want a cross-protocol read surface, point the handler at the matching VIEW rather than the per-protocol tables. If you want protocol-specific detail, point at the table directly.

### 5. Tests

Add fixture transactions under `core/tests/fixtures/` (one per recognized signature for the new protocol) and write integration tests in `core/tests/live_detector.rs`.

### What stays narrow

Each crate has exactly one dispatch point per protocol:

- `ExtractionDispatcher::extract` — one branch
- `EventPersistor::persist` — one branch
- `init_event_persistor` — one instantiation block

Everything else is per-protocol-isolated code. No central registry to update beyond these three points.

---

## Adding a new API endpoint

For endpoints that read existing data (no new tables, no new domain types), the workflow is contained in `api`:

### 1. Extend the relevant repository trait in `core`

If the endpoint needs a query that doesn't exist yet, add the method to the trait in `core/src/domain/<aggregate>/repository.rs`. Document the ordering and pagination contract.

### 2. Implement the new method in `persistence`

Add the SQL in the corresponding `Pg*Repository` impl. Regenerate `.sqlx/`.

### 3. Add the handler in `api`

- Create or extend a module under `api/src/http/handlers/`.
- Create request/response DTOs in `api/src/http/dto/`.
- Mount the route in `http/mod.rs::build_router`.
- Reuse `ApiError` for error mapping; the `From<RepositoryError>` impl handles repository failures uniformly.

### 4. Verify

```bash
cargo run -p yog-api
curl http://127.0.0.1:5000/api/<your-endpoint> | jq
```

### Conventions

- **Pagination** — all collection endpoints use cursor-based pagination via `Page<T>` and a domain-specific cursor type. Default `limit = 50`, hard cap `200`.
- **Error responses** — RFC 9457 Problem Details (see [Error responses](#error-responses) above).
- **Validation** — client-supplied data is validated at the handler boundary, before any DB call.
- **Pubkeys** — always serialized as base58 strings in responses (matching `Pubkey::Display`). Accept the same format on input.
- **Timestamps** — RFC3339 / ISO8601 (matching `chrono::DateTime<Utc>::Serialize` default).
