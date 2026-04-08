# yog-sothoth

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

## Roadmap

- [x] Project setup — Rust workspace, TimescaleDB, Next.js scaffold
- [ ] **Phase 1** — Indexer + `yog-core`: Solana WebSocket, DAMM v2 parser, AMM formulas
- [ ] **Phase 2** — WASM integration + minimal Next.js dashboard
- [ ] **Phase 3** — Full REST API, time-series persistence, configurable alerts
- [ ] **Phase 4** — Production dashboard, CI/CD, Clever Cloud deployment

---

## Contributing

Contributions are welcome. Please open an issue before submitting a pull request to discuss what you would like to change.

This project follows standard Rust and TypeScript conventions:

```bash
cargo fmt && cargo clippy   # Rust
npm run lint                # TypeScript
```

---

## License

[MIT](./LICENSE)