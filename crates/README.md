# crates/

This directory contains the Rust workspace — the core of yog-sothoth.

The workspace follows a **Domain-Driven Design** layout: business logic and contracts live in `core`, infrastructure and I/O live in dedicated adapter crates (`persistence` for Postgres, `bootstrap` for startup utilities). The two binaries (`indexer`, `api`) are thin assembly layers that wire the pieces together.

This README covers the inter-crate architecture and the responsibilities of each crate. For project-wide topics — three-process layout, database roles, pool observation model, hosting — see the [root README](../README.md).

---

## Structure

```
crates/
├── core/          ← shared library: domain types, AMM formulas, protocol parsing
├── persistence/   ← Postgres adapter: repository impls, migrations, query helpers
├── bootstrap/    ← shared startup utilities: env helpers, SecretUrl, init_rustls
├── indexer/       ← native binary: Solana RPC, transaction dispatch, event ingestion
├── api/           ← native binary: axum HTTP server over the indexed data
└── wasm/          ← WASM build target (scaffold, not yet functional)
```

The dependency graph is strict and one-directional:

```
                ┌──────────┐
                │   core   │  no I/O, wasm-compatible
                └────▲─────┘
                     │
        ┌────────────┼────────────┐
        │            │            │
   ┌────┴─────┐ ┌────┴─────┐ ┌────┴────┐
   │persistence│ │bootstrap │ │  wasm   │
   └────▲─────┘ └────▲─────┘ └─────────┘
        │            │
        └─────┬──────┘
              │
        ┌─────┴──────┐
        │            │
   ┌────┴────┐  ┌────┴────┐
   │ indexer │  │   api   │
   └─────────┘  └─────────┘
```

`core` knows nothing about Postgres, axum, or even the standard library's environment. It declares traits; adapters implement them. Both binaries depend only on `core` (for types) and the adapters they need.

---

## `core` (`yog-core`)

The shared library. Pure logic and domain types — no I/O, no runtime, no database.

### Layout

```
core/src/
├── domain/                           ← business entities + repository contracts
├── protocols/                        ← protocol-specific extraction
│   ├── anchor_event.rs               ← generic Anchor `event_cpi` decoder
│   ├── extraction.rs                 ← ExtractionOutcome, ExtractionFailure
│   ├── pool_indexer.rs               ← the `PoolIndexer` trait
│   └── meteora/
│       ├── damm_v2/                  (active — Phase 1)
│       ├── damm_v1.rs                (stub, Phase 2)
│       └── dlmm.rs                   (stub, Phase 2)
├── amm/                              ← pure AMM math (price, slippage, imbalance)
├── pagination.rs                     ← Page<T>, Cursor enum
└── error/                            ← CoreError, RepositoryError, CoreResult<T>
```

### Responsibilities

- **Domain models** (`domain/`) — entities (`Pool`, `SwapEvent`, `LiquidityEvent`, `ClaimPositionFeeEvent`, `ClaimRewardEvent`), the `DomainEvent` enum that unifies them, and the repository traits that define persistence contracts (`PoolRepository`, `SwapEventRepository`, …).
- **Protocol extraction** (`protocols/`) — per-protocol implementations of `PoolIndexer` that turn raw Solana transactions into typed domain events via Anchor `event_cpi` decoding.
- **AMM math** (`amm/`) — formulas for price, reserves, slippage, imbalance. Same Rust code targeted at native (today) and the browser via WASM (Phase 2), so computations cannot diverge between backend and frontend.
- **Pagination** (`pagination.rs`) — `Page<T>` envelope and discriminated `Cursor` enum used by every paginated repository method.
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
    ///   - `unknown`:  discriminators we don't recognize (other rings, future events)
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

The trait covers both write (indexer) and read (api) responsibilities — at runtime, the connected Postgres role determines which methods will actually succeed (the `yog_api` role lacks `INSERT/UPDATE` on event tables, so calling `upsert` from the api fails with `permission denied` from Postgres, by design — see [root README › Database roles](../README.md#database-roles)).

Concrete PostgreSQL implementations live in [`persistence`](#persistence-yog-persistence).

### Conventions and invariants

These invariants are documented on the affected types and enforced at construction time:

- **Mints sorted by raw bytes** — in `Pool`, `SwapEvent`, `LiquidityEvent`, `token_a_mint` and `token_b_mint` are ordered by `Pubkey::Ord` (raw bytes). Stable regardless of swap direction. Differs from the Meteora SDK canonical convention; documented on each affected struct.
- **Canonical `(token_a, token_b)` exposure** — `SwapEvent` and `LiquidityEvent` expose `amount_a` / `amount_b` and `reserve_a_after` / `reserve_b_after` in canonical order. Swap direction lives in the `TradeDirection` enum (`AtoB` | `BtoA`). Callers reconstruct the trader's perspective by combining the two.
- **`fee_token_is_a` precomputed** — boolean stored on `SwapEvent`, derived from `(collect_fee_mode, trade_direction)` in the translator. Mirrors `cp-amm::FeeMode::get_fee_mode`. Avoids recomputation at query time.
- **Four fee components separated** — `claiming_fee`, `protocol_fee`, `compounding_fee`, `referral_fee`. Lets v0.2 signal detectors (e.g. fee yield spike) distinguish LP yield from protocol revenue.
- **Lossless `u128` in DB** — `next_sqrt_price` (Q64.64) and `liquidity_delta` are stored as `NUMERIC(39, 0)`. Conversion via dedicated helpers in `persistence::repository_utils`.

### Compilation targets

- `cargo build` → native library, linked into `yog-indexer` and `yog-api` ✅
- `wasm-pack build` → WASM module for the browser 🚧 **not yet functional**

The `wasm` feature flag is declared in `Cargo.toml` but the required code-level changes (conditional `#[cfg(feature = "solana")]` on `amm` and `protocols`, abstracting `Pubkey`) are planned for Phase 2.

---

## `persistence` (`yog-persistence`)

Postgres adapter. Concrete implementations of the repository traits declared in `core`, plus the migrations and query helpers. No business logic.

### Layout

```
persistence/
├── migrations/                       ← sqlx migrations applied at deployment
│   └── 001_initial_schema.sql
├── setup_roles.sql                   ← one-time role provisioning (admin only)
└── src/
    ├── database.rs                   ← Database::connect, pool sizing
    ├── repository_utils.rs           ← string→Pubkey, u64↔i64, u128↔BigDecimal
    └── repositories/                 ← one impl per domain repository trait
        ├── pool.rs                   (PgPoolRepository)
        ├── swap_event.rs             (PgSwapEventRepository)
        ├── liquidity_event.rs        (PgLiquidityEventRepository)
        ├── position_fee_claim.rs     (PgPositionFeeClaimRepository)
        ├── reward_claim.rs           (PgRewardClaimRepository)
        └── watched_pool.rs           (PgWatchedPoolRepository)
```

### Responsibilities

- **Repository implementations** — one `Pg*Repository` struct per domain aggregate, each implementing the corresponding trait from `core::domain::*`. Constructors take a `PgPool`; the pool is owned by the consumer (each binary instantiates its own pool with its own role credentials).
- **Connection management** — `Database::connect(url)` returns a thin wrapper over `sqlx::PgPool` with sensible defaults (max 10 connections, 5s acquire timeout). Larger callers can use `connect_with_options` for explicit sizing.
- **Conversion helpers** (`repository_utils`) — `convert_string_to_pubkey`, `convert_u64_to_i64`, `convert_bigdecimal_to_u128`, etc. Uniform error mapping via `map_sqlx_error` which translates `sqlx::Error` variants into the right `RepositoryError` semantic (`NotFound`, `Conflict`, `Timeout`, `Backend`).
- **Schema migrations** — managed via `sqlx migrate`, source-of-truth at deployment time. Run by tooling or CI under the admin role; runtime processes never touch DDL.

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
    // sqlx::query! / query_as! against self.pool, errors mapped via map_sqlx_error,
    // row → domain conversion via repository_utils helpers.
}
```

Decode failures (malformed pubkey, unknown protocol) surface as `RepositoryError::Integrity` — they indicate schema corruption or an out-of-sync migration, not a runtime data issue.

### Why `Database` doesn't own the pool past construction

`Database::connect` builds a `PgPool` and returns a wrapper. Each binary then calls `database.pool().clone()` to hand the pool to repository constructors. `PgPool` is `Arc` internally, so cloning is cheap, and the `Database` wrapper can be dropped after wiring — the pool survives in each `PgXxxRepository` that holds a clone.

This shape makes the connection lifecycle explicit at the binary level. Each binary opens its own pool from its own `DATABASE_URL_*` (which embeds the role credentials), so process boundaries match role boundaries.

### SQLx offline cache

The crate uses `sqlx::query!` macros that verify SQL syntax against the live schema at compile time. The verified query cache is committed at the workspace level (`.sqlx/`), which allows the workspace to build in CI when `SQLX_OFFLINE=true`.

**After modifying any `sqlx::query!` call**, regenerate the cache before committing:

```bash
DATABASE_URL="$DATABASE_URL_ADMIN" \
SQLX_OFFLINE=false \
cargo sqlx prepare --workspace
```

The admin role is required because runtime roles (`yog_indexer`, `yog_api`) lack the introspection privileges sqlx needs across all tables.

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

The crate hosts utilities that are **identical across all native binaries**:

- Environment variable reading and parsing — every binary loads its own config from env vars, but the parsing primitives are shared.
- `SecretUrl` — a wrapper around connection strings whose `Display` and `Debug` impls redact the query string. Both binaries hold credentials in this type to prevent accidental leaks through logs or error chains.
- `ConfigError` — the canonical error type returned by every binary's `Config::load`. Two variants (`MissingVariable`, `InvalidValue`) cover all failure modes at this stage.
- `init_rustls()` — installs the rustls crypto provider, required by rustls 0.23+ before any TLS handshake.
- `init_tracing()` — configures the global tracing subscriber, switching between JSON and text output based on `LOG_FORMAT`.

### What does NOT go here

Things that vary across binaries stay in their respective binaries:

- The `Config` struct itself — the indexer's variables (`SOLANA_RPC_*`, `RPC_WORKER_MAX_RETRIES`, …) and the api's variables (`API_BIND_ADDR`, `DATABASE_URL_API`) don't overlap. A "shared config containing everyone's variables" is a smell, so each binary defines its own struct using the shared parsing helpers.
- `init_metrics` — the indexer exposes Prometheus on `:9000`; the api will expose its own metrics through axum middleware on its HTTP server with different histograms and labels. No symmetry to share.
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
├── bin/
│   └── debug_sig.rs                  ← one-shot signature inspection helper
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

Note that the database layer (`infra/db/` in earlier versions) has moved out — repository implementations now live in `crates/persistence/`, and the indexer consumes them like any other dependency.

### Three-stage pipeline

The indexer is structured as three Tokio tasks connected by bounded mpsc channels: `RpcListener` → `SignatureDispatcher` → `IndexerWorker` → `IndexerService`. Each stage has a single responsibility, its own typed error channel, and its own metrics.

The full diagram and rationale live in [root README › Indexer — three-stage pipeline](../README.md#indexer--three-stage-pipeline). This section covers what is specific to the indexer's internal organization.

**`IndexerWorker`** is the bridge between the channel-based pipeline and the per-signature processing. It applies bounded concurrency: a `Semaphore` with `MAX_CONCURRENT_INDEX_TASKS = 15` permits gates `tokio::spawn` of the per-signature indexing task. The receive loop applies natural back-pressure when all 15 slots are taken.

**`IndexerService`** drives the actual ingestion: fetch the transaction by signature (HTTP RPC), extract events via the matching `PoolIndexer` (one of `core`'s protocol implementations), persist to TimescaleDB through the repository traits.

### Skip-and-log error semantics

`IndexerService::index_transaction` follows a strict skip-and-log policy:

- **Per-event failures don't abort the others** — when persisting the events extracted from a single transaction, a failure on one event is logged, counted in `persist_failures_total{event_kind}`, and the next event is attempted.
- **Per-signature failures don't stop the worker** — the `IndexerWorker` catches errors from `index_transaction`, logs and counts them, and keeps draining the channel.
- **Loop-level failures bubble up** — closed channels, exhausted semaphores, panics in spawned tasks: these reach `Daemon::run` via typed `IndexerWorkerError` and trigger graceful shutdown of all three tasks via the shared `CancellationToken`.

The `ExitGuard` RAII helper in `IndexerService` ensures every entry into `index_transaction` produces an exit counter and duration sample — even on error paths that return early without explicitly tagging an outcome.

### Configuration

Reads its variables from the workspace `.env` (loaded by `dotenvy`):

```env
DATABASE_URL_INDEXER=postgresql://yog_indexer:...@host:5433/yog_sothoth
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
SOLANA_RPC_HTTP=https://api.mainnet-beta.solana.com
RPC_WORKER_MAX_RETRIES=10
MODE_PROTOCOL_CENTRIC=true
```

The env-var helpers (`required`, `parse_required_*`) come from `yog-bootstrap`. `Config::load()` lives in `bootstrap/config.rs` next to `Daemon::new`, grouping all startup concerns.

### Run

```bash
cargo run -p yog-indexer
```

Connects to Postgres as `yog_indexer` (RW on event tables, RO on `watched_pools`).

---

## `api` (`yog-api`)

Native binary. HTTP server built on axum 0.8 — exposes JSON endpoints over the indexed data.

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
│   │       └── pool_response.rs      (Pool wire shape)
│   ├── handlers/
│   │   ├── health.rs                 (/healthz)
│   │   └── pools.rs                  (/api/pools — list_pools)
│   ├── middleware.rs                 ← CORS, security headers
│   └── error.rs                      ← ApiError, IntoResponse, From<RepositoryError>
└── main.rs
```

### Responsibilities

- **HTTP routing and serving** (`http/mod.rs`) — builds the axum `Router`, applies the middleware stack, runs the serve loop on the address from `Config::bind_addr`.
- **Dependency container** (`bootstrap/app_state.rs`) — `AppState` holds shared dependencies as `Arc<dyn Trait>` references. `Clone` is cheap (everything is `Arc`-wrapped), which axum requires for the `State` extractor.
- **Handlers** (`http/handlers/`) — one module per route family. Handlers are pure async functions taking axum extractors (`State<AppState>`, `Query<T>`) and returning `Result<Json<T>, ApiError>`.
- **Response DTOs** (`http/dto/response/`) — wire shapes decoupled from the domain. `PoolResponse` formats pubkeys as base58 strings; `PageResponse<T>` is the generic envelope for paginated responses.
- **Error type** (`http/error.rs`) — `ApiError` with three variants (`BadRequest`, `NotFound`, `Internal`) plus an `IntoResponse` impl. Internal errors are logged with full context but never expose implementation details to the client.
- **Middleware** (`http/middleware.rs`) — CORS (permissive in dev, to be tightened once the Next.js dashboard is deployed), security headers (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`).

(reste de la section inchangé : Pattern for handlers, Cursor wire format, Configuration, Run)
### Pattern for handlers

```rust
pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PoolsQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let cursor = decode_cursor(query.cursor.as_deref())?;
    let page = state.pool_repository.find_paginated(cursor, query.limit).await?;

    Ok(Json(PageResponse {
        items: page.items.into_iter().map(PoolResponse::from).collect(),
        next_cursor: page.next_cursor.as_ref().map(encode_cursor).transpose()?,
    }))
}
```

The handler signature is the contract: extractors describe what the handler needs, the return type describes what it produces. Body content goes through axum's `Json<T>` which sets `Content-Type: application/json` automatically.

### Cursor wire format

Pagination cursors are **opaque to clients**: a base64(url-safe, no-pad) encoding of a JSON-serialized `PoolCursorWire` struct. Clients pass back the `next_cursor` from the previous response without interpreting it. The wire format is intentionally JSON-based to keep cursors debuggable by hand if needed (decode the base64, read the JSON), and to allow extending the cursor structure without breaking compatibility.

The encoding/decoding lives next to the handler that uses it (`handlers/pools.rs`); when more domains become paginated (swap events, liquidity events), each will define its own `XxxCursorWire` struct.

### Configuration

Reads its variables from the workspace `.env`:

```env
DATABASE_URL_API=postgresql://yog_api:...@host:5433/yog_sothoth
API_BIND_ADDR=127.0.0.1:3000
```

`bind_addr` is parsed as `SocketAddr` at load time — typo in the env var fails fast with a clear `ConfigError::InvalidValue`, before any task is spawned.

### Run

```bash
cargo run -p yog-api
```

Connects to Postgres as `yog_api` (RO on event tables today; will gain `INSERT/UPDATE` on user-facing tables in v0.3).

---

## `wasm` (`yog-wasm`)

WebAssembly target for the browser. **Currently a scaffold** — the default `cargo new --lib` template, not yet wired to `yog-core`.

Making it functional requires:

1. Activating the `wasm` feature on `yog-core` (currently a placeholder).
2. Conditional compilation (`#[cfg(feature = "solana")]`) on modules that pull Solana-only crates — `solana-pubkey`, `solana-transaction-status`, etc. These do not compile for `wasm32-unknown-unknown` without significant configuration (`getrandom` backend selection, among others).
3. Abstracting `Pubkey` behind a neutral type alias so the `domain/` layer compiles on both targets.

Scheduled for **Phase 2**.

---

## Building the workspace

```bash
# Build all native crates
cargo build

# Build a specific crate
cargo build -p yog-core
cargo build -p yog-persistence
cargo build -p yog-indexer
cargo build -p yog-api

# Run tests (workspace-wide, sqlx in offline mode)
SQLX_OFFLINE=true cargo test --workspace --all-features

# Lint
cargo clippy --workspace --all-targets --all-features

# Format
cargo fmt --all
```

For the full local CI checklist, lint policy, and the SQL query regeneration workflow, see the [root README › Development](../README.md#development).

---

## Adding a new protocol

The workflow follows the existing Meteora DAMM v2 layout. A new protocol typically introduces new event types, which means changes in three crates.

### 1. In `core`

- Create a module under `core/src/protocols/<family>/<protocol>/` (e.g. `protocols/meteora/dlmm/`).
- Split responsibilities across files following the DAMM v2 pattern: `events.rs` for wire events (borsh mirrors of on-chain Anchor events), `extractor.rs` for walking the transaction's inner instructions, `translator.rs` for the wire → domain translation.
- Create a top-level struct (e.g. `MeteoraDlmm`) and implement `PoolIndexer` — concretely, `extract_events` chains your extractor and translator.
- If the protocol introduces domain events that don't yet exist (e.g. DLMM bin-specific events), add them to `core/src/domain/` with their model and repository trait.

### 2. In `persistence`

- If the protocol introduces new tables, add a migration under `persistence/migrations/` (numbered after the latest one). Include `GRANT INSERT, UPDATE ON <new_table> TO yog_indexer;` for any table the indexer must write to.
- Implement the new repository traits in `persistence/src/repositories/` following the existing pattern.
- Regenerate `.sqlx/` with the admin role.

### 3. In `indexer`

- Register the implementation in `IndexerService::protocol_indexer` (the dispatch site that maps `Protocol` → `Arc<dyn PoolIndexer>`).
- If new events were added in `core`, wire their persistence in `IndexerService::index_transaction` — the dispatch from `DomainEvent` variant to repository call.

### 4. In `api` (when read access is needed)

- Add new endpoints in `api/src/axum_app/handlers/` (one module per resource family).
- Add the corresponding repository to `AppState` if it isn't already there.
- Add request/response DTOs as needed.

### 5. Tests

Add fixture transactions under `core/tests/fixtures/` and write integration tests in `core/tests/live_detector.rs` against real captured signatures.

The `PoolIndexer` contract is uniform across protocols, so adding a new one is mostly a matter of providing the extractor/translator pair — no central dispatch table to maintain.

---

## Adding a new API endpoint

For endpoints that read existing data (no new tables, no new domain types), the workflow is contained in `api`:

### 1. Extend the relevant repository trait in `core`

If the endpoint needs a query that doesn't exist yet (e.g. `find_by_protocol`, `find_active_in_window`), add the method to the trait in `core/src/domain/<aggregate>/repository.rs`. Document the ordering and pagination contract.

### 2. Implement the new method in `persistence`

Add the SQL in the corresponding `Pg*Repository` impl. If you introduce a new query, regenerate `.sqlx/` with the admin role.

### 3. Add the handler in `api`

- Create or extend a module under `api/src/axum_app/handlers/`.
- Create request/response DTOs in the same module (or a sibling `dto.rs` if the handler grows).
- Mount the route in `axum_app/mod.rs::build_router`.
- Reuse `ApiError` for error mapping; the `From<RepositoryError>` impl handles repository failures uniformly.

### 4. Verify

```bash
cargo run -p yog-api
curl http://127.0.0.1:3000/api/<your-endpoint> | jq
```

### Conventions

- **Pagination** — all collection endpoints use cursor-based pagination via `Page<T>` and a domain-specific cursor type. Default `limit = 50`, hard cap `200`.
- **Error responses** — JSON shape `{ "error": "<message>" }`, HTTP status from `ApiError` variant. Internal errors are logged but the message returned to the client is generic (`"internal server error"`) — never expose query details, schema names, or DB driver errors.
- **Validation** — client-supplied data is validated at the handler boundary, before any DB call. Limit out of range, malformed cursor, missing required field → `ApiError::BadRequest` with a descriptive message.
- **Pubkeys** — always serialized as base58 strings in responses (matching `Pubkey::Display`). Accept the same format on input.
- **Timestamps** — RFC3339 / ISO8601 (matching `chrono::DateTime<Utc>::Serialize` default).