# crates/

This directory contains the Rust workspace — the core of yog-sothoth.

---

## Structure

```
crates/
├── core/        ← shared library: domain types, AMM formulas, protocol parsing
├── indexer/     ← native binary: Solana RPC, transaction dispatch, persistence
└── wasm/        ← WASM build target (scaffold, not yet functional — see below)
```

The workspace follows a **Domain-Driven Design** layout: business logic and contracts
live in `core`, infrastructure and I/O live in `indexer`. The two are connected through
traits defined in the domain layer and implemented in the infra layer.

---

## Crates

### `core` (`yog-core`)

The shared library. Pure logic and domain types — no I/O, no runtime, no database.

#### Layout

```
core/src/
├── domain/                           ← business entities + repository contracts
│   ├── pool/                         (model + repository)
│   ├── swap_event/                   (model + repository)
│   ├── liquidity_event/              (model + repository)
│   ├── claim_position_fee_event/     (model + repository)
│   ├── claim_reward_event/           (model + repository)
│   ├── watched_pool/                 (model + repository)
│   ├── protocol/                     (model only)
│   ├── domain_event.rs               (enum over all event variants)
│   └── trade_direction.rs            (AtoB | BtoA)
├── protocols/                        ← protocol-specific extraction
│   ├── anchor_event.rs               ← generic Anchor `event_cpi` decoder
│   ├── extraction.rs                 ← ExtractionOutcome, ExtractionFailure
│   ├── pool_indexer.rs               ← the `PoolIndexer` trait
│   └── meteora/
│       ├── damm_v2/                  (active — Phase 1)
│       │   ├── events.rs             ← wire events (borsh mirrors)
│       │   ├── extractor.rs          ← walks inner_instructions
│       │   └── translator.rs         ← wire → domain translation
│       ├── damm_v1.rs                (stub, Phase 2)
│       └── dlmm.rs                   (stub, Phase 2)
├── amm/                              ← pure AMM math (price, slippage, imbalance)
└── error/                            ← `CoreError`, `RepositoryError`, `CoreResult<T>`
```

#### Responsibilities

- **Domain models** (`domain/`) — entities (`Pool`, `SwapEvent`, `LiquidityEvent`,
  `ClaimPositionFeeEvent`, `ClaimRewardEvent`), the `DomainEvent` enum that
  unifies them, and the repository traits that define persistence contracts.
- **Protocol extraction** (`protocols/`) — per-protocol implementations of
  `PoolIndexer` that turn raw Solana transactions into typed domain events
  via Anchor `event_cpi` decoding.
- **AMM math** (`amm/`) — formulas for price, reserves, slippage, imbalance.
  Target: same Rust code runs native and in the browser (via WASM) so
  computations cannot diverge between backend and frontend.

#### Compilation targets

- `cargo build` → native library, linked into `yog-indexer` ✅
- `wasm-pack build` → WASM module for the browser 🚧 **not yet functional**

The `wasm` feature flag is declared in `Cargo.toml` but the required code-level
changes (conditional `#[cfg(feature = "solana")]` on `amm` and `protocols`,
abstracting `Pubkey`) are planned for Phase 2.

#### Supported protocols

| Protocol | Status |
|---|---|
| Meteora DAMM v2 | **Active** — Cercle 1 events (`Swap2`, `LiquidityChange`, `ClaimPositionFee`, `ClaimReward`) implemented end-to-end |
| Meteora DAMM v1 | Stub — `extract_events` returns empty `ExtractionOutcome` |
| Meteora DLMM | Stub — `extract_events` returns empty `ExtractionOutcome` |

Cercle 2 events (`CreatePosition`, `ClosePosition`, `InitializePool`, …) and
Cercle 3 (admin / config) are not yet wired but will fit the same pipeline
without architectural changes — only new wire-event mirrors and translator
arms to add.

#### The `PoolIndexer` trait

Every protocol implementation exposes a single extraction entry point. The
indexer dispatches transactions to the correct implementation based on
`Protocol` (resolved upstream by the dispatcher).

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

**Why a single method instead of `is_swap` / `parse_swap` / …?** The previous
`transferChecked`-based parser needed instruction-type discrimination because
each variant had a different account ordering. The current pipeline decodes
Anchor `event_cpi` payloads, which carry their type in an 8-byte discriminator —
one pass over the inner instructions yields every recognized event in one go.
Per-instruction multiplexing is no longer the right shape.

#### Anchor `event_cpi` extraction pipeline

Each Meteora program emits its events via Anchor's `emit_cpi!` mechanism — a
self-CPI to an `event_authority` PDA, with a stable wire format:

```
[8 bytes EVENT_IX_TAG][8 bytes event discriminator][borsh payload]
```

where `EVENT_IX_TAG = sha256("anchor:event")[..8]` is the fixed prefix injected
by Anchor.

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

Three failure types are distinguished in `ExtractionFailure` and counted as
separate metric labels: `AnchorDecode` (prefix or payload-size mismatch),
`Borsh` (schema mismatch), `Translation` (missing transferChecked context,
invalid enum value).

#### Conventions and invariants

These invariants are documented on the affected types and enforced at
construction time:

- **Mints sorted by raw bytes** — in `Pool`, `SwapEvent`, `LiquidityEvent`,
  `token_a_mint` and `token_b_mint` are ordered by `Pubkey::Ord` (raw bytes).
  Stable regardless of swap direction. Differs from the Meteora SDK canonical
  convention; documented on each affected struct.
- **Canonical `(token_a, token_b)` exposure** — `SwapEvent` and `LiquidityEvent`
  expose `amount_a` / `amount_b` and `reserve_a_after` / `reserve_b_after` in
  canonical order. Swap direction lives in the `TradeDirection` enum
  (`AtoB` | `BtoA`). Callers reconstruct the trader's perspective by combining
  the two.
- **`fee_token_is_a` precomputed** — boolean stored on `SwapEvent`, derived
  from `(collect_fee_mode, trade_direction)` in the translator. Mirrors
  `cp-amm::FeeMode::get_fee_mode`. Avoids recomputation at query time.
- **Four fee components separated** — `claiming_fee`, `protocol_fee`,
  `compounding_fee`, `referral_fee`. Lets v0.2 signal detectors (e.g. fee
  yield spike) distinguish LP yield from protocol revenue.
- **Lossless `u128` in DB** — `next_sqrt_price` (Q64.64) and `liquidity_delta`
  are stored as `NUMERIC(39, 0)`. Conversion via dedicated helpers in
  `repository_utils.rs` on the indexer side.

#### Repository traits

Each domain aggregate that needs persistence declares a repository trait in
its module — e.g. `domain/swap_event/repository.rs`:

```rust
#[async_trait]
pub trait SwapEventRepository: Send + Sync {
    async fn insert(&self, event: &SwapEvent) -> Result<(), RepositoryError>;
    // ... query methods added as the dashboard needs them
}
```

The concrete PostgreSQL/TimescaleDB implementations live in
`indexer/src/infra/db/repositories/`. The application layer in `indexer`
depends on the trait, not the implementation — standard dependency inversion.

---

### `indexer` (`yog-indexer`)

The native binary. Runs as a long-lived process in production.

#### Layout

```
indexer/src/
├── application/
│   ├── services/
│   │   ├── indexer_service.rs        ← fetch → extract → persist pipeline
│   │   ├── watched_pool_service.rs   ← allowlist management
│   │   ├── errors.rs                 ← typed internal errors (FetchError)
│   │   └── metrics.rs                ← Prometheus metric definitions
│   └── workers/
│       ├── indexer.rs                ← bounded-concurrency consumer of signatures
│       └── subscription.rs           ← WebSocket subscription supervisor
├── bin/
│   └── debug_sig.rs                  ← one-shot signature inspection helper
├── bootstrap/
│   └── daemon.rs                     ← top-level lifecycle, task wiring, shutdown
├── config/
│   └── secret_url.rs                 ← redacted URL wrapper for logs
├── error/                            ← typed error per layer (5 modules)
├── infra/
│   ├── db/
│   │   ├── database.rs               ← connection pool
│   │   ├── repository_utils.rs       ← u128 ↔ NUMERIC helpers
│   │   └── repositories/             ← impls of core's repository traits
│   └── rpc/
│       ├── dispatcher/               ← log-event → qualified-signature filtering
│       │   ├── filters/              (failed_transaction, invocation, watched_pool)
│       │   └── metrics.rs
│       ├── types/                    (qualified_signature, raw_log_event)
│       └── listener.rs               ← WebSocket subscription client
├── utils/
│   └── redact.rs                     ← API-key scrubbing for logs
└── main.rs
```

#### Three-stage pipeline

The indexer is structured as three Tokio tasks connected by bounded mpsc
channels. Each stage has a single responsibility and a typed error channel:

```
┌──────────────┐    raw    ┌──────────────────┐  qualified  ┌────────────────┐
│ RpcListener  │──────────▶│ SignatureDispat. │────────────▶│ IndexerWorker  │
│              │  RawLog   │                  │  Signature  │                │
│ logsSubscribe│  Events   │ filter chain     │  + protocol │ ↓ spawn task   │
│ + reconnect  │           │ (failed / invoc. │             │ ↓ semaphore-   │
│              │           │  / watched_pool) │             │   bounded      │
└──────────────┘           └──────────────────┘             └────────┬───────┘
                                                                     │
                                                                     ▼
                                                            ┌────────────────┐
                                                            │ IndexerService │
                                                            │ fetch → extract│
                                                            │ → persist      │
                                                            └────────────────┘
```

**`RpcListener`** owns the WebSocket connection to the Solana RPC, handles
reconnection with exponential backoff, and forwards raw log events downstream.

**`SignatureDispatcher`** applies a chain of filters that turn raw log events
into `QualifiedSignature`s — `(protocol, signature)` pairs that have passed the
failed-transaction check, the protocol-invocation check, and the watched-pool
allowlist (currently a temporary RPC-throughput constraint, see root README).

**`IndexerWorker`** consumes qualified signatures and drives `IndexerService`
with bounded concurrency. The cap is `MAX_CONCURRENT_INDEX_TASKS = 15`,
calibrated against the Helius free tier (10 req/s) with headroom. A semaphore
gates `tokio::spawn` of the per-signature indexing task; the receive loop
applies natural back-pressure when all 15 slots are taken.

#### Skip-and-log error semantics

`IndexerService::index_transaction` follows a strict skip-and-log policy:

- **Per-event failures don't abort the others** — when persisting the events
  extracted from a single transaction, a failure on one event is logged,
  counted in `persist_failures_total{event_kind}`, and the next event is
  attempted.
- **Per-signature failures don't stop the worker** — the `IndexerWorker`
  catches errors from `index_transaction`, logs and counts them, and keeps
  draining the channel.
- **Loop-level failures bubble up** — closed channels, exhausted semaphores,
  panics in spawned tasks: these reach `Daemon::run` via typed
  `IndexerWorkerError` and trigger graceful shutdown of all three tasks via
  the shared `CancellationToken`.

The `ExitGuard` RAII helper in `IndexerService` ensures every entry into
`index_transaction` produces an exit counter and duration sample — even on
error paths that return early without explicitly tagging an outcome.

#### Configuration

Via environment variables (loaded by `dotenvy`):

```env
DATABASE_URL=postgresql://user:pasword@localhost:5433/database_name
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
SOLANA_RPC_HTTP=https://api.mainnet-beta.solana.com
```

#### Run

```bash
cargo run -p yog-indexer
```

#### SQLx compile-time verification

The indexer uses `sqlx::query!` macros that verify SQL syntax against the
live schema at compile time. The verified query cache is committed to
`crates/indexer/.sqlx/`, which allows the workspace to build in CI (or
anywhere without a running database) when `SQLX_OFFLINE=true` is set.

**After modifying any `sqlx::query!` call**, regenerate the cache before
committing:

```bash
cd crates/indexer
cargo sqlx prepare
```

Otherwise CI will fail.

---

### `wasm` (`yog-wasm`)

WebAssembly target for the browser. **Currently a scaffold** — the default
`cargo new --lib` template, not yet wired to `yog-core`.

Making it functional requires:

1. Activating the `wasm` feature on `yog-core` (currently a placeholder).
2. Conditional compilation (`#[cfg(feature = "solana")]`) on modules that
   pull Solana-only crates — `solana-pubkey`, `solana-transaction-status`,
   etc. These do not compile for `wasm32-unknown-unknown` without
   significant configuration (`getrandom` backend selection, among others).
3. Abstracting `Pubkey` behind a neutral type alias so the `domain/` layer
   compiles on both targets.

Scheduled for **Phase 2**.

---

## Building the workspace

```bash
# Build all native crates
cargo build

# Build a specific crate
cargo build -p yog-core
cargo build -p yog-indexer

# Run tests (indexer requires SQLX_OFFLINE unless DATABASE_URL is set)
SQLX_OFFLINE=true cargo test -p yog-core -p yog-indexer --all-features

# Lint
cargo clippy -p yog-core -p yog-indexer --all-targets --all-features

# Format
cargo fmt --all
```

For the full local CI checklist and lint policy, see the
[root README](../README.md#development).

---

## Adding a new protocol

The workflow follows the existing Meteora DAMM v2 layout:

1. Create a module under `core/src/protocols/<family>/<protocol>/`
   (e.g. `protocols/meteora/dlmm/` when you're ready to tackle it).
2. Split responsibilities across files following the DAMM v2 pattern:
   `events.rs` for wire events (borsh mirrors of on-chain Anchor events),
   `extractor.rs` for walking the transaction's inner instructions,
   `translator.rs` for the wire → domain translation.
3. Create a top-level struct (e.g. `MeteoraDlmm`) and implement `PoolIndexer`
   — concretely, `extract_events` chains your extractor and translator.
4. Register the implementation in `IndexerService::protocol_indexer`
   (the dispatch site that maps `Protocol` → `Arc<dyn PoolIndexer>`).
5. Add fixture transactions under `core/tests/fixtures/` and write
   integration tests in `core/tests/live_detector.rs` against real
   captured signatures.

The `PoolIndexer` contract is uniform across protocols, so adding a new one
is a matter of providing the extractor/translator pair — no central
dispatch table to maintain.