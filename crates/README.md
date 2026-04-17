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
├── domain/              ← business entities and repository contracts (pure DDD)
│   ├── liquidity_event/
│   ├── pool_metric/
│   ├── pool_state/
│   ├── protocol/
│   ├── swap_event/
│   └── watched_pool/
├── protocols/           ← protocol-specific parsing and detection
│   ├── meteora/
│   │   ├── damm_v1.rs   (scaffold, not yet wired)
│   │   ├── damm_v2/     (active — Phase 1)
│   │   │   ├── detector.rs   ← instruction-type discriminants
│   │   │   ├── parser.rs     ← swap/liquidity event extraction
│   │   │   ├── reserves.rs   ← reserve balance computation
│   │   │   └── transfer.rs   ← SPL token transfer parsing
│   │   └── dlmm.rs      (scaffold, not yet wired)
│   └── pool_indexer.rs  ← the `PoolIndexer` trait
├── amm/                 ← pure AMM math (x·y=k, slippage, etc.)
└── error/               ← `CoreError`, `CoreResult<T>` alias
```

#### Responsibilities

- **Domain models** (`domain/`) — entities like `SwapEvent`, `LiquidityEvent`,
  `PoolState`, plus the `Repository` traits that define persistence contracts.
- **Protocol parsing** (`protocols/`) — per-protocol implementations of `PoolIndexer`
  that turn raw Solana transactions into domain events.
- **AMM math** (`amm/`) — formulas for price, reserves, slippage, imbalance.
  Target: same Rust code runs native and in the browser (via WASM) so computations
  cannot diverge between backend and frontend.

#### Compilation targets

- `cargo build` → native library, linked into `yog-indexer` ✅
- `wasm-pack build` → WASM module for the browser 🚧 **not yet functional**

The `wasm` feature flag is declared in `Cargo.toml` but the required code-level
changes (conditional `#[cfg(feature = "solana")]` on `amm` and `protocols`,
abstracting `Pubkey`) are planned for Phase 2.

#### Supported protocols

| Protocol | Status |
|---|---|
| Meteora DAMM v2 | **Active** — detector, parser, reserves, transfer implemented |
| Meteora DAMM v1 | Scaffold only — empty struct, not wired to `PoolIndexer` |
| Meteora DLMM | Scaffold only — empty struct, not wired to `PoolIndexer` |

#### The `PoolIndexer` trait

Every protocol parser implements this trait. The indexer dispatches transactions to
the correct implementation based on `program_id()`.

```rust
pub trait PoolIndexer: Send + Sync {
    fn program_id(&self) -> Pubkey;

    // Discriminants — called first by the indexer
    fn is_swap(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool;
    fn is_add_liquidity(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool;
    fn is_remove_liquidity(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool;

    // Parsers — called only after the matching discriminant returns true
    fn parse_swap(&self, tx: &EncodedConfirmedTransactionWithStatusMeta)
        -> CoreResult<SwapEvent>;
    fn parse_add_liquidity(&self, tx: &EncodedConfirmedTransactionWithStatusMeta)
        -> CoreResult<LiquidityEvent>;
    fn parse_remove_liquidity(&self, tx: &EncodedConfirmedTransactionWithStatusMeta)
        -> CoreResult<LiquidityEvent>;
}
```

**Dispatch contract:** the indexer always calls a discriminant (`is_*`) before its
corresponding parser (`parse_*`). Implementations may assume this ordering and
skip redundant instruction-type checks inside `parse_*`.

**Error semantics:** parsers return `CoreResult<T>` (not `Option<T>`) so that
"not my protocol" and "malformed transaction" are distinguishable error states.

#### Repository traits

Each domain aggregate that needs persistence declares a repository trait in its
module — e.g. `domain/swap_event/repository.rs`:

```rust
#[async_trait]
pub trait SwapEventRepository: Send + Sync {
    async fn insert(&self, event: &SwapEvent) -> CoreResult<()>;
    async fn find_by_pool(&self, pool_address: &Pubkey, limit: i64)
        -> CoreResult<Vec<SwapEvent>>;
}
```

The concrete PostgreSQL/TimescaleDB implementation lives in
`indexer/src/infra/db/repositories/`. The application layer in `indexer` depends
on the trait, not the implementation — standard dependency inversion.

---

### `indexer` (`yog-indexer`)

The native binary. Runs as a long-lived process in production.

#### Layout

```
indexer/src/
├── application/         ← use cases (orchestration, no I/O directly)
│   └── services/
│       ├── indexer_service.rs
│       └── watch_pool_service.rs
├── bootstrap/           ← daemon entry point, lifecycle management
│   └── daemon.rs
├── infra/               ← concrete I/O — DB and RPC adapters
│   ├── db/
│   │   ├── database.rs          ← connection pool
│   │   └── repositories/        ← impls of core's repository traits
│   └── rpc/
│       └── websocket.rs         ← Solana WebSocket client
├── error/               ← crate-local error types
├── config.rs            ← env-var loading
└── main.rs
```

#### Responsibilities

- Open a WebSocket subscription to Solana RPC
- Receive raw transactions for watched pool addresses
- Dispatch each transaction to the correct `PoolIndexer` implementation from `core`
- Persist resulting domain events through the repository traits (implemented in `infra/db/`)

#### Configuration

Via environment variables (loaded by `dotenvy`):

```env
DATABASE_URL=postgresql://yog:yog@localhost:5433/yog_sothoth
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
```

#### Run

```bash
cargo run -p yog-indexer
```

#### SQLx compile-time verification

The indexer uses `sqlx::query!` macros that verify SQL syntax against the live
schema at compile time. The verified query cache is committed to `crates/indexer/.sqlx/`,
which allows the workspace to build in CI (or anywhere without a running database)
when `SQLX_OFFLINE=true` is set.

**After modifying any `sqlx::query!` call**, regenerate the cache before committing:

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

1. Activating the `wasm` feature on `yog-core` (currently a placeholder)
2. Conditional compilation (`#[cfg(feature = "solana")]`) on modules that pull
   Solana-only crates — `solana-pubkey`, `solana-transaction-status`, etc. These
   do not compile for `wasm32-unknown-unknown` without significant configuration
   (`getrandom` backend selection, among others).
3. Abstracting `Pubkey` behind a neutral type alias so the `domain/` layer can
   compile on both targets.

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
   (e.g. `protocols/meteora/dlmm/` when you're ready to tackle it)
2. Split responsibilities across files — at minimum: `detector.rs` for
   instruction discriminants, `parser.rs` for event extraction.
   Add `reserves.rs`, `transfer.rs`, or others as the protocol demands.
3. Create a top-level struct (e.g. `MeteoraDlmm`) and implement `PoolIndexer` for it.
4. Register the implementation wherever the indexer resolves parsers by `program_id`.
5. Add fixture transactions under `core/tests/fixtures/` and write parser tests.

The indexer dispatches by `program_id()`, so no central dispatch table needs to
be modified — registration is local to wherever you plug the new parser in.