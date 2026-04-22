# yog-sothoth

![CI](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/ci.yml/badge.svg)

> Real-time liquidity analysis engine for Meteora DEX pools on Solana.

*Yog-Sothoth is the entity that sees everything simultaneously — past, present, future, all planes of existence at once. A fitting name for a tool that continuously observes the on-chain transaction stream, reconstructs pool state over time, and detects patterns in DeFi liquidity flows.*

---

## What it is

yog-sothoth ingests Solana transactions in real time, reconstructs the state of Meteora AMM pools, computes financial metrics, and exposes alerts and visualizations through a web dashboard.

This is **not** a block explorer (≠ Solscan).
It is an analysis and signal tool — somewhere between Dune Analytics and Nansen, focused on Meteora liquidity.

---

## Features

- 🔴 **Real-time indexing** — WebSocket RPC connection, live transaction parsing
- 📐 **AMM reconstruction** — price, reserves, slippage, imbalance computed from on-chain state
- 📊 **Time-series metrics** — historical data stored in TimescaleDB, queryable over sliding windows
- ⚡ **WASM in the browser** — same Rust AMM formulas run client-side via WebAssembly
- 🔔 **Configurable alerts** — threshold-based notifications per pool (price impact, TVL drop, imbalance)
- 🌐 **REST + WebSocket API** — real-time push to the dashboard, no polling

---

## Stack

| Layer | Technology | Role |
|---|---|---|
| Indexer | **Rust** | Transaction parsing, pool state reconstruction, DB writes |
| AMM engine | **Rust → WASM** | Shared `yog-core` crate compiled for both native and browser |
| Frontend + API | **Next.js / TypeScript** | Dashboard UI + API Routes (replaces a separate Node.js process) |
| Database | **TimescaleDB** | Time-series storage, sliding window queries, automatic compression |
| Transport | **WebSocket** | Solana RPC (indexer) + real-time push to browser (Next.js) |

---

## Architecture

```
┌─────────────────────────────────────────┐
│              Production                 │
│                                         │
│  process 1 : indexer (Rust)             │
│  └── WebSocket → Solana RPC             │
│  └── Parse transactions (DAMM v2 first) │
│  └── Write → TimescaleDB                │
│                                         │
│  process 2 : web (Next.js)              │
│  └── Dashboard UI                       │
│  └── API Routes (read TimescaleDB)      │
│  └── WebSocket push → browser           │
│  └── Configurable alerts                │
│                                         │
│  TimescaleDB                            │
│  └── sole communication channel         │
│      between the two processes          │
└─────────────────────────────────────────┘
```

The two processes communicate **only through the database** — the indexer writes, Next.js reads. No direct inter-process calls.

### Repository structure

```
yog-sothoth/
├── crates/
│   ├── core/        ← shared Rust crate (AMM formulas, protocol parsing)
│   ├── indexer/     ← native binary (Solana RPC, DB writes)
│   └── wasm/        ← WASM build of core/ for the browser
└── web/             ← Next.js dashboard + API Routes
```

### The `yog-core` crate — dual compilation target

The `core` crate contains AMM formulas and transaction parsing logic.
It compiles to two targets from the same source:

- `cargo build` → native binary used by the indexer
- `wasm-pack build` → WASM module loaded by Next.js

Price and slippage calculations are **identical** between backend and frontend — no possible divergence.

---

## Supported protocols

| Priority | Protocol | Model |
|---|---|---|
| **Phase 1** | Meteora DAMM v2 | x·y=k + dynamic fees + NFT positions |
| Phase 2 | Meteora DLMM | Bin-based liquidity, volatility fees |
| Phase 2 | Meteora DAMM v1 | x·y=k + dual-yield (lending) |
| Phase 3+ | DAMM v1 Farm, Stake2Earn, LST, Multi-Token | — |

---

## Getting started

### Prerequisites

- Rust (stable) — [rustup.rs](https://rustup.rs)
- `wasm-pack` — [rustwasm.github.io/wasm-pack](https://rustwasm.github.io/wasm-pack)
- Node.js LTS — via [nvm](https://github.com/nvm-sh/nvm)
- Docker (for TimescaleDB)

### Run locally

```bash
# Clone
git clone https://github.com/sicotjeanvivien/yog-sothoth.git
cd yog-sothoth

# Start TimescaleDB
docker compose up -d

# Stop and delete container
docker compose down -v

# Start Indexer
cargo run --bin indexer

# Build the Rust workspace
cargo build

# Build the WASM module
wasm-pack build crates/wasm --target web

# Install and run Next.js
cd web
npm install
npm run dev
```

### Environment variables

Copy `.env.example` to `.env` and fill in:

```env
DATABASE_URL=postgresql://yog:yog@localhost:5433/yog_sothoth
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
```

---

## Development

### Workspace layout

The Rust workspace is organized into three crates with clear separation of concerns:

- **`yog-core`** — domain logic, AMM formulas, protocol parsing. Compiles native and (eventually) WASM.
- **`yog-indexer`** — native-only binary. Solana WebSocket listener, TimescaleDB persistence via SQLx.
- **`yog-wasm`** — browser-facing wrapper around `yog-core`. Currently a scaffold (see *Feature flags* below).

### Feature flags

`yog-core` exposes two features to support its dual compilation target:

| Feature | Default | Purpose |
|---|---|---|
| `solana` | ✅ | Pulls in `solana-pubkey`, `solana-transaction-status`. Required by `yog-indexer`. |
| `wasm` | — | Reserved for the browser build. **Not yet functional.** |

The `wasm` feature is currently a placeholder. Activating it will require conditional
compilation (`#[cfg(feature = "solana")]`) on the `amm` and `protocols` modules, plus
abstracting `Pubkey` behind a neutral type alias. Scheduled for **Phase 2**.

### Continuous integration

GitHub Actions runs on every push and pull request to `main`:

- **Format** — `cargo fmt --all -- --check` (strict)
- **Lint** — `cargo clippy -p yog-core -p yog-indexer --all-targets --all-features` (warnings non-blocking during initial development)
- **Tests** — `cargo test -p yog-core -p yog-indexer --all-features` (strict)

SQLx compile-time query verification runs in offline mode via `SQLX_OFFLINE=true`,
using the committed `.sqlx/` cache. **When you modify an `sqlx::query!` call, regenerate
the cache** before committing:

```bash
cd crates/indexer
cargo sqlx prepare
```

WASM builds are not part of CI yet — see *Feature flags* above.

### Local checks before pushing

The CI mirrors these three commands. Run them locally to catch issues before pushing:

```bash
cargo fmt --all -- --check
cargo clippy -p yog-core -p yog-indexer --all-targets --all-features
SQLX_OFFLINE=true cargo test -p yog-core -p yog-indexer --all-features
```

### Lint policy

Clippy warnings are currently **non-blocking** in CI. The project is in an early
architectural phase with deliberate scaffolding (e.g. unused `program_id_str` fields
anticipating future protocol support). `-D warnings` will be reactivated once the
core architecture stabilizes (target: end of Phase 1).

---

## Roadmap

- [x] Project setup — Rust workspace, TimescaleDB, Next.js scaffold
- [x] CI pipeline — GitHub Actions (fmt, clippy, tests) with SQLx offline verification
- [ ] **Phase 1** — Indexer + `yog-core`: Solana WebSocket, DAMM v2 parser, AMM formulas
- [ ] **Phase 2** — WASM integration + minimal Next.js dashboard
- [ ] **Phase 3** — Full REST API, time-series persistence, configurable alerts
- [ ] **Phase 4** — Production dashboard, CD, Clever Cloud deployment

---

## Contributing

Contributions are welcome. Please open an issue before submitting a pull request to discuss what you would like to change.

### Rust conventions

All Rust code must pass the checks enforced by CI (see *Development > Continuous integration*):

```bash
cargo fmt --all              # Format code
cargo clippy --workspace     # Lint (warnings visible, non-blocking)
cargo test --workspace       # Run tests
```

### TypeScript conventions

```bash
cd web
npm run lint
```

### Before opening a PR

1. Run the three local CI commands listed in *Development > Local checks*
2. If you modified an `sqlx::query!` call, regenerate `.sqlx/` with `cargo sqlx prepare`
3. Open your PR against `main` — GitHub Actions will run automatically

---

## License

[MIT](./LICENSE)





 cargo run --bin debug_sig -- 2yUdrWx2TmdL9SLk3AKaSpv54BLtMSeTUihGwqc1UhgPbUVkuZR11KvioTJFkUYwQjH1MSbpEMn64a7EaTY7BTmS
  cargo run --release --bin yog-indexer 2>&1 | tee log/indexer-helius.log