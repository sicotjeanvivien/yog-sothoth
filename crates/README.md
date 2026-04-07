# crates/

This directory contains the Rust workspace — the core of yog-sothoth.

---

## Structure

```
crates/
├── core/        ← shared library: AMM formulas, protocol parsing, domain types
├── indexer/     ← native binary: Solana RPC connection, transaction ingestion, DB writes
└── wasm/        ← WASM build target: exposes yog-core to the browser via WebAssembly
```

---

## Crates

### `core` (`yog-core`)

The shared library. Contains everything that is protocol logic and math — no I/O, no side effects.

**Responsibilities:**
- AMM formulas: current price, reserves, slippage, pool imbalance
- Transaction parsing: swap events, liquidity add/remove events
- Protocol trait `PoolIndexer` — implemented per supported protocol
- Domain types shared across the workspace

**Compilation targets:**
- `cargo build` → compiled as a native library, linked into `indexer`
- `wasm-pack build` → compiled as a WASM module, loaded by the Next.js frontend

The same formulas run on both the backend and the browser. No divergence possible.

**Supported protocols:**

| Protocol | Status |
|---|---|
| Meteora DAMM v2 | Phase 1 — implemented |
| Meteora DLMM | Phase 2 — stub |
| Meteora DAMM v1 | Phase 2 — stub |

**The `PoolIndexer` trait:**

```rust
pub trait PoolIndexer {
    fn parse_swap(&self, tx: &Transaction) -> Option<SwapEvent>;
    fn parse_add_liquidity(&self, tx: &Transaction) -> Option<LiquidityEvent>;
    fn parse_remove_liquidity(&self, tx: &Transaction) -> Option<LiquidityEvent>;
    fn program_id(&self) -> Pubkey;
}
```

Each protocol is a concrete implementation of this trait. Adding a new protocol means implementing `PoolIndexer` — nothing else changes.

---

### `indexer`

The native binary. Runs as a long-lived process in production.

**Responsibilities:**
- WebSocket connection to Solana RPC
- Subscribe to configured pool addresses
- Dispatch raw transactions to the appropriate `PoolIndexer` implementation
- Write parsed events and metrics to TimescaleDB

**Configuration** (via environment variables):
```env
DATABASE_URL=postgresql://yog:yog@localhost:5433/yog_sothoth
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
```

**Run:**
```bash
cargo run -p indexer
```

---

### `wasm`

The WebAssembly build of `yog-core`. Thin wrapper that re-exports `core` functions with `#[wasm_bindgen]` annotations for JavaScript interop.

**Build:**
```bash
wasm-pack build crates/wasm --target web --out-dir ../../web/public/wasm
```

The output is placed directly in `web/public/wasm/` so Next.js can load it as a static asset.

---

## Building the workspace

```bash
# Build all crates (native)
cargo build

# Run tests
cargo test

# Lint
cargo clippy

# Format
cargo fmt
```

---

## Adding a new protocol

1. Create a new file in `core/src/protocols/` (e.g. `meteora_dlmm.rs`)
2. Implement the `PoolIndexer` trait
3. Register the implementation in `core/src/protocols/mod.rs`
4. The indexer will dispatch transactions to it automatically based on `program_id()`
