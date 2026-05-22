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

---

## Structure

```
crates/
├── core/          ← shared library: domain types, AMM math, protocol decoding
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
├── domain/                           ← entities + repository contracts
│   ├── pool/                         (Pool, PoolRepository)
│   ├── swap_event/                   (SwapEvent, SwapEventRepository)
│   ├── liquidity_event/              (LiquidityEvent, LiquidityEventRepository)
│   ├── claim/                        (ClaimPositionFeeEvent, ClaimRewardEvent, ...)
│   ├── token_metadata/               (TokenMetadata, TokenMetadataRepository)
│   ├── token_price/                  (TokenPrice, PriceSource, TokenPriceRepository)
│   ├── network_status/               (NetworkStatus, NetworkStatusRepository)
│   ├── pool_current_state/           (PoolCurrentState, repository)
│   └── watched_pool/                 (WatchedPool, WatchedPoolRepository)
├── protocols/                        ← per-protocol extraction
│   ├── anchor_event.rs               ← generic Anchor `event_cpi` decoder
│   ├── extraction.rs                 ← ExtractionOutcome, ExtractionFailure
│   ├── pool_indexer.rs               ← the `PoolIndexer` trait
│   └── meteora/
│       ├── damm_v2/                  (active — v0.1)
│       ├── damm_v1.rs                (stub, v0.5)
│       └── dlmm.rs                   (stub, v0.5)
├── amm/                              ← pure AMM math (price, slippage, imbalance)
├── pagination.rs                     ← Page<T>, Cursor enum
├── solana_types.rs                   ← re-export hub for solana SDK types
└── error/                            ← CoreError, RepositoryError, CoreResult<T>
```

### Responsibilities

- **Domain models** (`domain/`) — entities, the `DomainEvent` enum that unifies the indexer's events, and the repository traits that define every persistence contract (`PoolRepository`, `SwapEventRepository`, `TokenMetadataRepository`, …).
- **Protocol extraction** (`protocols/`) — per-protocol implementations of `PoolIndexer` that turn raw Solana transactions into typed domain events via Anchor `event_cpi` decoding.
- **AMM math** (`amm/`) — formulas for price, reserves, slippage, imbalance. Kept here because they will eventually run in the browser too via WASM (deferred — see [`wasm`](#wasm-yog-wasm)).
- **Pagination** (`pagination.rs`) — `Page<T>` envelope and discriminated `Cursor` enum used by every paginated repository method.
- **Solana SDK indirection** (`solana_types.rs`) — single point of contact for types reshuffled by Solana SDK releases (`EncodedConfirmedTransactionWithStatusMeta`, `UiInstruction`, `option_serializer`). When the SDK restructures, only this file changes.
- **Errors** (`error/`) — `CoreError` for domain-level failures, `RepositoryError` as the boundary type returned by every repository trait. Adapters convert their internal errors (e.g. `sqlx::Error`) into `RepositoryError` at their public surface.

### The `PoolIndexer` trait

Every protocol implementation exposes a single extraction entry point. The indexer dispatches transactions to the correct implementation based on `Protocol` (resolved upstream by the dispatcher).

```rust
pub trait PoolIndexer: Send + Sync {
    /// Program ID this indexer handles, as a base58 string.
    fn program_id(&self) -> &str;

    /// Extract every domain event the transaction emitted for this protocol.
    ///
    /// Returns an `ExtractionOutcome` carrying:
    ///   - `events`:   successfully extracted and translated domain events
    ///   - `unknown`:  discriminators we don't recognize (other circles, future events)
    ///   - `failures`: events we recognized but failed to translate
    ///
    /// Never returns `Err` at the transaction level — partial failure is reported
    /// via `failures`.
    fn extract_events(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome>;
}
```

**Why a single method instead of `is_swap` / `parse_swap` / …?** The previous `transferChecked`-based parser needed instruction-type discrimination because each variant had a different account ordering. The current pipeline decodes Anchor `event_cpi` payloads, which carry their type in an 8-byte discriminator — one pass over the inner instructions yields every recognized event in one go. Per-instruction multiplexing is no longer the right shape.

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
[anchor_event.rs]   extract_anchor_event_cpis(tx, program_id)
        │           ├─ iterates over inner_instructions
        │           ├─ filters: programId match + EVENT_IX_TAG prefix
        │           └─ returns Vec<Vec<u8>>  (decoded base58 payloads)
        ▼
[damm_v2/events.rs] match discriminator → DammV2WireEvent::{...}
        │           └─ borsh::deserialize the payload
        ▼
[damm_v2/translator.rs] translate_wire_event(wire, transfer_checked_group, ...)
        │           ├─ for Swap2 / LiquidityChange: extract mints from surrounding transferChecked
        │           ├─ compute_fee_token_is_a from (collect_fee_mode, trade_direction)
        │           └─ returns DomainEvent
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

The trait covers both write (indexer) and read (api) responsibilities — at runtime, the connected Postgres role determines which methods actually succeed. The `yog_api` role lacks `INSERT/UPDATE` on event tables, so calling `upsert` from the api fails with `permission denied` from Postgres itself, by design (see [Database roles](#database-roles)).

### Conventions and invariants

These invariants are documented on the affected types and enforced at construction time:

- **Mints sorted by raw bytes** — in `Pool`, `SwapEvent`, `LiquidityEvent`, `token_a_mint` and `token_b_mint` are ordered by `Pubkey::Ord` (raw bytes). Stable regardless of swap direction. Differs from the Meteora SDK canonical convention; documented on each affected struct.
- **Canonical `(token_a, token_b)` exposure** — `SwapEvent` and `LiquidityEvent` expose `amount_a` / `amount_b` and `reserve_a_after` / `reserve_b_after` in canonical order. Swap direction lives in the `TradeDirection` enum (`AtoB` | `BtoA`). Callers reconstruct the trader's perspective by combining the two.
- **`fee_token_is_a` precomputed** — boolean stored on `SwapEvent`, derived from `(collect_fee_mode, trade_direction)` in the translator. Mirrors `cp-amm::FeeMode::get_fee_mode`. Avoids recomputation at query time.
- **Four fee components separated** — `claiming_fee`, `protocol_fee`, `compounding_fee`, `referral_fee`. Lets v0.2 signal detectors (e.g. fee yield spike) distinguish LP yield from protocol revenue.
- **Lossless `u128` in DB** — `next_sqrt_price` (Q64.64) and `liquidity_delta` are stored as `NUMERIC(39, 0)`. Conversion via dedicated helpers in `persistence::repository_utils`.
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
├── migrations/                       ← sqlx migrations (forward-only)
│   ├── 001_initial_schema.sql        (consolidated v0.1 baseline)
│   └── README.md                     (forward-only convention, GRANT policy)
├── setup_roles.sql                   ← one-time role provisioning (admin)
└── src/
    ├── database.rs                   ← Database::connect, run_migrations
    ├── repository_utils.rs           ← string→Pubkey, u64↔i64, u128↔BigDecimal
    ├── repositories/                 ← one impl per domain repository trait
    │   ├── pool.rs                   (PgPoolRepository)
    │   ├── swap_event.rs             (PgSwapEventRepository)
    │   ├── liquidity_event.rs        (PgLiquidityEventRepository)
    │   ├── position_fee_claim.rs     (PgPositionFeeClaimRepository)
    │   ├── reward_claim.rs           (PgRewardClaimRepository)
    │   ├── pool_current_state.rs     (PgPoolCurrentStateRepository)
    │   ├── network_status.rs         (PgNetworkStatusRepository)
    │   ├── token_metadata.rs         (PgTokenMetadataRepository)
    │   ├── token_price.rs            (PgTokenPriceRepository)
    │   └── watched_pool.rs           (PgWatchedPoolRepository)
    └── bin/
        └── migrate.rs                ← yog-migrate binary (~30 lines)
```

### Responsibilities

- **Repository implementations** — one `Pg*Repository` per domain aggregate. Each takes a `PgPool` in its constructor; the pool is owned by the consumer (each binary instantiates its own pool with its own role credentials).
- **Connection management** — `Database::connect(url)` returns a thin wrapper over `sqlx::PgPool` with sensible defaults (max 10 connections, 5s acquire timeout). `Database::run_migrations()` exposes `sqlx::migrate!()` behind a clean signature so the `yog-migrate` binary can call a domain method, not sqlx directly.
- **Conversion helpers** (`repository_utils`) — `convert_string_to_pubkey`, `convert_u64_to_i64`, `convert_bigdecimal_to_u128`, etc. Uniform error mapping via `map_sqlx_error` which translates `sqlx::Error` variants into the right `RepositoryError` semantic (`NotFound`, `Conflict`, `Timeout`, `Backend`).
- **Schema migrations** (`migrations/`) — sqlx-managed, source of truth at deployment time. Applied by `yog-migrate` (a binary) or `cargo sqlx migrate run` (in CI / locally), both running under the `yog_migrate` DDL role.

### The `yog-migrate` binary

`crates/persistence/src/bin/migrate.rs` is a small (~30 LOC) one-shot binary:

```bash
cargo run -p yog-persistence --bin yog-migrate
```

It reads `DATABASE_URL_MIGRATE` from the environment, connects under the `yog_migrate` role, applies any pending migration via `Database::run_migrations()`, and exits 0. In Docker, it runs once at compose-up time; runtime services depend on it via `service_completed_successfully` so they never start against a half-migrated schema.

`yog-migrate` is the **only** path through which DDL flows in production. The four runtime roles (`yog_indexer`, `yog_api`, `yog_context`, and the legacy `yog`-superuser admin) cannot CREATE or ALTER tables — by design.

### Migrations conventions

The `migrations/` directory follows two rules. The detailed rationale lives in [`migrations/README.md`](./persistence/migrations/README.md); the short version:

- **Forward-only.** Migrations committed to git never change. No `.down.sql`. Rollback in production is a backup restore; rollback in development is a `pg_dump` taken before applying.
- **GRANTs live in the migration that creates the table.** Each `CREATE TABLE` is followed by its `GRANT INSERT, UPDATE` (and any other non-default) statements. `SELECT` is covered by the default privileges set in `setup_roles.sql`. This keeps the table and its permission contract in the same versioned file.

### `setup_roles.sql`

Slim provisioning script applied once per database as superuser. It creates the four runtime roles, transfers `public` schema ownership to `yog_migrate`, and sets `ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate` so the tables created by future migrations inherit the right `SELECT` grants automatically. **It contains no table-specific GRANTs** — those live in the migrations.

### Database roles

Four service roles plus an admin role, enforced at the database level:

| Role | Permissions | Used by |
|---|---|---|
| `yog_migrate` | DDL — owns the schema, applies migrations | `yog-migrate` binary, `cargo sqlx migrate run` |
| `yog_indexer` | `SELECT, INSERT, UPDATE` on event tables; `SELECT` on `watched_pools` | indexer process |
| `yog_api` | `SELECT` across event, enrichment, and `watched_pools` tables | api process |
| `yog_context` | `SELECT, INSERT, UPDATE` on `token_metadata` and `token_prices`; `SELECT` on `pools` | context process |
| admin (e.g. `yog` superuser) | Full — provisioning, `cargo sqlx prepare`, ad-hoc operations | tooling only, never a running service |

A bug or compromise in the api cannot corrupt event data — Postgres rejects the operation before the SQL is ever sent. Future tables that need write access from new components require an explicit `GRANT` per migration; default privileges grant `SELECT` automatically to enforce a conscious decision per table for `INSERT/UPDATE`.

### Pattern for repository implementations

Each implementation follows the same structure:

```rust
pub struct PgPoolRepository {
    pool: PgPool,
}

impl PgPoolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolRepository for PgPoolRepository {
    // sqlx::query! / query_as! against self.pool,
    // errors mapped via map_sqlx_error,
    // row → domain conversion via repository_utils helpers.
}
```

Decode failures (malformed pubkey, unknown protocol) surface as `RepositoryError::Integrity` — they indicate schema corruption or an out-of-sync migration, not a runtime data issue.

### SQLx offline cache

The crate uses `sqlx::query!` macros that verify SQL syntax against the live schema at compile time. The verified query cache is committed under `crates/persistence/.sqlx/`, which allows the workspace to build everywhere with `SQLX_OFFLINE=true` — including Docker, CI, and any machine without DB access.

**After modifying any `sqlx::query!` call**, regenerate the cache before committing:

```bash
cd crates/persistence
cargo sqlx prepare
```

(Reads `DATABASE_URL` from the environment; use the admin role since runtime roles lack the introspection privileges sqlx needs across all tables.)

CI runs `cargo sqlx prepare --check` in a dedicated `sqlx-check` job against a real Postgres with migrations applied — see [CI](#ci).

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

Utilities **identical across all native binaries**:

- Environment variable reading and parsing — every binary loads its own config from env vars, but the parsing primitives are shared.
- `SecretUrl` — a wrapper around connection strings whose `Display` and `Debug` impls redact the query string. Every binary holds credentials in this type to prevent accidental leaks through logs or error chains.
- `ConfigError` — the canonical error type returned by every binary's `Config::load`. Two variants (`MissingVariable`, `InvalidValue`) cover all failure modes at this stage.
- `init_rustls()` — installs the rustls crypto provider, required by rustls 0.23+ before any TLS handshake.
- `init_tracing()` — configures the global tracing subscriber, switching between JSON and text output based on `LOG_FORMAT`.

### What does NOT go here

Things that vary across binaries stay in their respective binaries:

- The `Config` struct itself — the indexer's variables (`SOLANA_RPC_*`, `RPC_WORKER_MAX_RETRIES`, …), the api's (`API_BIND_ADDR`, `DATABASE_URL_API`) and the context's (`JUPITER_URL`, `CONTEXT_*_SECS`) don't overlap. A "shared config containing everyone's variables" is a smell, so each binary defines its own struct using the shared parsing helpers.
- `init_metrics` — the indexer exposes Prometheus on `:9000`; the api will expose its own metrics through axum middleware with different histograms and labels. No symmetry to share.
- Process-specific signal handling, shutdown logic, dependency wiring.

The decision rule when adding a new utility: **does this run identically in every binary's `main()`?** If yes, it belongs in `bootstrap`. If it varies even slightly, it stays in the binary.

---

## `indexer` (`yog-indexer`)

Native binary. Long-lived process consuming Solana mainnet WebSocket events and persisting indexed data.

### Layout

```
indexer/src/
├── application/
│   ├── services/
│   │   ├── indexer_service.rs        ← fetch → extract → persist
│   │   ├── watched_pool_service.rs   ← allowlist management
│   │   ├── errors.rs                 ← typed internal errors (FetchError)
│   │   └── metrics.rs                ← Prometheus metric definitions
│   └── workers/
│       ├── indexer.rs                ← bounded-concurrency consumer
│       └── subscription.rs           ← WebSocket subscription supervisor
├── bootstrap/
│   ├── config.rs                     ← Config::load() — env-driven configuration
│   └── daemon.rs                     ← top-level lifecycle, task wiring, shutdown
├── error/                            ← typed error per layer (5 modules)
├── infra/
│   └── rpc/
│       ├── dispatcher/               ← log-event → qualified-signature filtering
│       ├── types/                    (qualified_signature, raw_log_event)
│       └── listener.rs               ← WebSocket subscription client
├── utils/
│   └── redact.rs                     ← API-key scrubbing for logs
└── main.rs
```

The database layer is not here — repository implementations live in `crates/persistence/`, and the indexer consumes them like any other dependency.

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
                                                            ┌────────────────┐
                                                            │ IndexerService │
                                                            │ fetch → extract│
                                                            │ → persist      │
                                                            └────────────────┘
```

**`RpcListener`** owns the WebSocket connection, handles reconnection with exponential backoff, and forwards raw log events downstream. It is itself an orchestrator of a fleet of `SubscriptionWorker` instances (one per pool in the allowlist), each with its own retry budget (`RPC_WORKER_MAX_RETRIES`).

**`SignatureDispatcher`** applies a chain of filters that turn raw log events into qualified `(protocol, signature)` pairs — drops failed transactions, transactions that don't actually invoke the watched protocol, and (temporarily) transactions outside the watched-pool allowlist.

**`IndexerWorker`** consumes qualified signatures and drives `IndexerService` with bounded concurrency. The cap is `MAX_CONCURRENT_INDEX_TASKS = 15`, calibrated against the Helius free tier (10 req/s) with headroom. Per-signature failures are logged and counted but never stop the pipeline; loop-level failures (closed channels, exhausted semaphore, panics) bubble up to the daemon and trigger graceful shutdown via a shared `CancellationToken`.

**`IndexerService`** drives the actual ingestion: fetch the transaction by signature (HTTP RPC), extract events via the matching `PoolIndexer` from `core`, persist to TimescaleDB through the repository traits.

### Skip-and-log error semantics

`IndexerService::index_transaction` follows a strict skip-and-log policy:

- **Per-event failures don't abort the others** — when persisting the events extracted from a single transaction, a failure on one event is logged, counted in `persist_failures_total{event_kind}`, and the next event is attempted.
- **Per-signature failures don't stop the worker** — the `IndexerWorker` catches errors from `index_transaction`, logs and counts them, and keeps draining the channel.
- **Loop-level failures bubble up** — closed channels, exhausted semaphores, panics in spawned tasks reach `Daemon::run` via typed `IndexerWorkerError` and trigger graceful shutdown of all three tasks via the shared `CancellationToken`.

The `ExitGuard` RAII helper ensures every entry into `index_transaction` produces an exit counter and duration sample — even on error paths that return early without explicitly tagging an outcome.

### Observability

The indexer exposes Prometheus metrics on `:9000/metrics`. Key families:

- **Pipeline counters** — `raw_log_events_total`, `raw_log_events_rejected_total{filter, reason}`, `qualified_signatures_total`, `downstream_saturated_total`
- **Service counters** — `index_transaction_entered/exited_total{outcome}`, `events_indexed_total{event_kind}`, `transactions_no_match_total`, `unknown_event_total{discriminator}`, `extraction_failure_total{kind}`
- **Persistence** — `persist_failures_total{event_kind}`, `fetch_failures_total{reason}`, `fetch_not_found_total`
- **Allowlist filter** — `watched_pool_filter_passed_total{pool_address}`, `watched_pool_filter_dropped_total`
- **Histograms** — `fetch_duration_seconds`, `persist_duration_seconds{kind}`, `index_transaction_duration_seconds{outcome}`
- **Gauges** — `watched_pools_active`

### Configuration

Reads its variables from the workspace `.env`:

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
│   ├── app_state.rs                  ← AppState — dependency container
│   └── config.rs                     ← Config::load() — env-driven configuration
├── error/                            ← reserved for future api-specific errors
├── http/
│   ├── dto/
│   │   └── response/
│   │       ├── page_response.rs      (PageResponse<T> envelope)
│   │       ├── pool_response.rs      (Pool wire shape with embedded tokens)
│   │       ├── token_response.rs     (Token wire shape with embedded price)
│   │       └── ...
│   ├── handlers/
│   │   ├── health.rs                 (/healthz)
│   │   ├── pools.rs                  (/api/pools — list_pools)
│   │   ├── tokens.rs                 (/api/tokens/{mint} — get_token)
│   │   └── network_status.rs         (/api/network/status)
│   ├── middleware.rs                 ← CORS, security headers
│   └── error.rs                      ← ApiError, IntoResponse, From<RepositoryError>
└── main.rs
```

### Responsibilities

- **HTTP routing and serving** — builds the axum `Router`, applies the middleware stack, runs the serve loop on the address from `Config::bind_addr`.
- **Dependency container** (`AppState`) — holds shared dependencies as `Arc<dyn Trait>` references. `Clone` is cheap (everything is `Arc`-wrapped), which axum requires for the `State` extractor.
- **Handlers** — one module per route family. Pure async functions taking axum extractors (`State<AppState>`, `Query<T>`, `Path<T>`) and returning `Result<Json<T>, ApiError>`.
- **Response DTOs** — wire shapes decoupled from the domain. `PoolResponse` formats pubkeys as base58 strings and embeds the two tokens via `EmbeddedTokenResponse`; `PageResponse<T>` is the generic envelope for paginated responses.
- **Error type** — `ApiError` with `BadRequest`, `NotFound`, `Internal` variants plus an `IntoResponse` impl. Internal errors are logged with full context but never expose implementation details to the client.
- **Middleware** — CORS (permissive in dev, to be tightened once the dashboard is deployed), security headers (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`).

### Current endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness probe (200 OK, no DB roundtrip) |
| `GET` | `/api/network/status` | Latest indexer/RPC slot, RPC latency, observed timestamp |
| `GET` | `/api/pools` | Paginated list of discovered pools (cursor-based, `limit` 1–200, default 50). `PoolResponse` embeds `tokenA` and `tokenB` as `EmbeddedTokenResponse` |
| `GET` | `/api/tokens/{mint}` | Single token (metadata + latest price). 404 if metadata unknown; 200 with `price: null` if metadata exists but no price yet |

### Pattern for handlers

```rust
pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PoolsQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let cursor = decode_cursor(query.cursor.as_deref())?;
    let page = state.pool_repository.find_paginated(cursor, query.limit).await?;

    let mut items = Vec::with_capacity(page.items.len());
    for pool in page.items {
        items.push(enrich_pool(&state, pool).await?);
    }

    Ok(Json(PageResponse {
        items,
        next_cursor: page.next_cursor.as_ref().map(encode_cursor).transpose()?,
    }))
}
```

The handler signature is the contract: extractors describe what the handler needs, the return type describes what it produces. Body content goes through axum's `Json<T>` which sets `Content-Type: application/json` automatically.

### Cursor wire format

Pagination cursors are **opaque to clients**: a base64(url-safe, no-pad) encoding of a JSON-serialized `*CursorWire` struct. Clients pass back the `next_cursor` from the previous response without interpreting it. JSON keeps cursors debuggable by hand (decode the base64, read the JSON) and allows extending the structure without breaking compatibility.

When more domains become paginated (swap events, liquidity events), each will define its own `XxxCursorWire` struct.

### Configuration

```env
DATABASE_URL_API=postgresql://yog_api:...@host:5433/yog_sothoth
API_BIND_ADDR=127.0.0.1:5000
```

`bind_addr` is parsed as `SocketAddr` at load time — typos fail fast with a clear `ConfigError::InvalidValue`, before any task is spawned. The Docker compose service overrides this to `0.0.0.0:5000` automatically.

### Run

```bash
cargo run -p yog-api
```

Connects to Postgres as `yog_api` (RO across the board; will gain `INSERT/UPDATE` on user-facing tables in v0.3).

---

## `context` (`yog-context`)

Native binary. Token enrichment daemon — fills in symbol / name / decimals / logo for every mint observed by the indexer, and refreshes USD prices on a regular cadence.

### Layout

```
context/src/
├── application/
│   ├── metadata_worker.rs            ← polls list_missing_mints, fetches DAS, upserts
│   └── price_worker.rs               ← polls list_known_mints, fetches Jupiter, inserts
├── bootstrap/
│   ├── config.rs                     ← Config::load() — env-driven configuration
│   └── daemon.rs                     ← spawns both workers, owns the shutdown token
├── infra/
│   ├── helius_das.rs                 ← Helius DAS getAssetBatch client
│   └── jupiter_price.rs              ← Jupiter Price V3 client
└── main.rs
```

### Two workers, two cadences

The daemon spawns two independent worker loops:

- **Metadata worker** — every `CONTEXT_METADATA_POLL_SECS` (default 10s), queries `TokenMetadataRepository::list_missing_mints` for mints present in `pools` but absent from `token_metadata`. Batches them (up to 1000, the Helius DAS `getAssetBatch` cap), fetches metadata, upserts in one batch insert. Mints with no `token_info.decimals` in the DAS response are skipped (incomplete metadata is treated as "no metadata" rather than poisoning the table with NULLs everywhere).
- **Price worker** — every `CONTEXT_PRICE_INTERVAL_SECS` (default 30s), queries `TokenMetadataRepository::list_known_mints` and asks Jupiter Price V3 for current USD prices. Batches at the Jupiter cap (50 mints per request). Inserts one row per `(mint, price)` pair into the `token_prices` hypertable. Mints with no price (Jupiter returned `usdPrice: null` or omitted the field) are skipped silently — the next tick retries.

### Resilience

Both workers are **deliberately resilient**: HTTP errors, decode errors, and per-row persistence errors are absorbed inside the loop (logged and counted, then `continue`). The worker never propagates an error out — only the daemon's shutdown token can stop a worker. This matches the indexer's skip-and-log doctrine: third-party API outages must not crash the enrichment process.

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

Making it functional requires:

1. Activating a `wasm` feature on `yog-core` (currently a placeholder).
2. Conditional compilation (`#[cfg(feature = "solana")]`) on modules that pull Solana-only crates — `solana-pubkey`, `solana-transaction-status`, etc. These do not compile for `wasm32-unknown-unknown` without significant configuration (`getrandom` backend selection, among others).
3. Abstracting `Pubkey` behind a neutral type alias so the `domain/` layer compiles on both targets.

Deferred to **v0.3**, where the decision will be reassessed in light of concrete frontend use cases (interactive swap simulation, signal preview). As of v0.1, no such use case justifies the chain; the events already carry `reserves`, `sqrt_price` and `fees` natively, so dashboard rendering needs no client-side AMM math.

---

## Local development

Two workflows are supported — pick whichever fits the task.

### A. Docker (default, easiest)

The full stack lives in `docker-compose.yml` at the repo root. Bring it up with the `backend` profile and everything runs:

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

The first build is long (~15-25 min) because `cargo-chef` cooks every workspace dependency from scratch. Subsequent builds reuse the cooked layer and drop to 30-90 seconds.

### B. Native `cargo run` (faster inner loop)

Useful when iterating on a single binary's code. Postgres runs in Docker, the binary runs natively:

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

The four `DATABASE_URL_*` variables in `.env` point to `localhost:5433` for this workflow. The docker-compose services rewrite the host to `postgres:5432` automatically on startup, so the same `.env` works for both A and B.

### Building, testing, linting

```bash
# Build all native crates
cargo build

# Build a specific crate
cargo build -p yog-core
cargo build -p yog-context

# Run tests (workspace-wide, sqlx in offline mode)
cargo test --workspace --all-features

# Lint
cargo clippy -p yog-api -p yog-core -p yog-context -p yog-indexer -p yog-persistence \
    --all-targets --all-features -- -D warnings

# Format
cargo fmt --all
```

The Rust version is pinned in `rust-toolchain.toml` at the repo root (`1.86.0` as of this writing) and read automatically by `cargo`, `dtolnay/rust-toolchain@stable` in CI, and IDE integrations. You don't pick a Rust version — the file does.

---

## CI

GitHub Actions runs on every push and pull request to `main`, in two workflows:

- **`crates.yml`** — Rust workspace:
  - `check` — `cargo check --workspace --all-targets`
  - `fmt` — `cargo fmt --all -- --check` (strict)
  - `clippy` — `cargo clippy ... -- -D warnings` (strict, native crates only)
  - `test` — `cargo test ... --all-features`
  - `audit` — `cargo audit` against Cargo.lock; documented ignores in `.cargo/audit.toml`
  - `sqlx-check` — spins up a TimescaleDB service, applies migrations, runs `cargo sqlx prepare --check` to verify the committed `.sqlx/` cache matches the queries in the source
- **`web-quality.yml`** — Next.js typecheck, lint, vitest

The `sqlx-check` job is the safety net for the offline cache: if you add or modify a `query!()` call and forget to run `cargo sqlx prepare`, the job fails with a clear pointer to the fix.

---

## Adding a new protocol

The workflow follows the existing Meteora DAMM v2 layout. A new protocol typically introduces new event types, which means changes in three crates.

### 1. In `core`

- Create a module under `core/src/protocols/<family>/<protocol>/` (e.g. `protocols/meteora/dlmm/`).
- Split responsibilities following the DAMM v2 pattern: `events.rs` for wire events (borsh mirrors of on-chain Anchor events), `extractor.rs` for walking the transaction's inner instructions, `translator.rs` for the wire → domain translation.
- Create a top-level struct (e.g. `MeteoraDlmm`) and implement `PoolIndexer` — `extract_events` chains your extractor and translator.
- If the protocol introduces domain events that don't yet exist (e.g. DLMM bin-specific events), add them to `core/src/domain/` with their model and repository trait.

### 2. In `persistence`

- If the protocol introduces new tables, add a migration under `persistence/migrations/` (numbered after the latest one). Include `GRANT INSERT, UPDATE ON <new_table> TO yog_indexer;` for any table the indexer writes to. `SELECT` is automatic via default privileges.
- Implement the new repository traits in `persistence/src/repositories/` following the existing pattern.
- Regenerate `.sqlx/` (`cd crates/persistence && cargo sqlx prepare`).

### 3. In `indexer`

- Register the implementation in `IndexerService::protocol_indexer` (the dispatch site that maps `Protocol` → `Arc<dyn PoolIndexer>`).
- If new events were added in `core`, wire their persistence in `IndexerService::index_transaction`.

### 4. In `api` (when read access is needed)

- Add new endpoints in `api/src/http/handlers/` (one module per resource family).
- Add the corresponding repository to `AppState` if it isn't already there.
- Add request/response DTOs.

### 5. Tests

Add fixture transactions under `core/tests/fixtures/` and write integration tests in `core/tests/live_detector.rs` against real captured signatures.

The `PoolIndexer` contract is uniform across protocols, so adding a new one is mostly providing the extractor/translator pair — no central dispatch table to maintain.

---

## Adding a new API endpoint

For endpoints that read existing data (no new tables, no new domain types), the workflow is contained in `api`:

### 1. Extend the relevant repository trait in `core`

If the endpoint needs a query that doesn't exist yet (e.g. `find_by_protocol`, `find_active_in_window`), add the method to the trait in `core/src/domain/<aggregate>/repository.rs`. Document the ordering and pagination contract.

### 2. Implement the new method in `persistence`

Add the SQL in the corresponding `Pg*Repository` impl. Regenerate `.sqlx/` if the query is new.

### 3. Add the handler in `api`

- Create or extend a module under `api/src/http/handlers/`.
- Create request/response DTOs in the same module (or a sibling `dto.rs` if the handler grows).
- Mount the route in `http/mod.rs::build_router`.
- Reuse `ApiError` for error mapping; the `From<RepositoryError>` impl handles repository failures uniformly.

### 4. Verify

```bash
cargo run -p yog-api
curl http://127.0.0.1:5000/api/<your-endpoint> | jq
```

### Conventions

- **Pagination** — all collection endpoints use cursor-based pagination via `Page<T>` and a domain-specific cursor type. Default `limit = 50`, hard cap `200`.
- **Error responses** — JSON shape `{ "error": "<message>" }`, HTTP status from `ApiError` variant. Internal errors are logged but the message returned to the client is generic (`"internal server error"`) — never expose query details, schema names, or DB driver errors.
- **Validation** — client-supplied data is validated at the handler boundary, before any DB call. Limit out of range, malformed cursor, missing required field → `ApiError::BadRequest` with a descriptive message.
- **Pubkeys** — always serialized as base58 strings in responses (matching `Pubkey::Display`). Accept the same format on input.
- **Timestamps** — RFC3339 / ISO8601 (matching `chrono::DateTime<Utc>::Serialize` default).