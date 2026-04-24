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

## Pool observation model

### Long-term target — protocol-centric

The target design is **protocol-centric**: the indexer subscribes directly to the Meteora program IDs and ingests every transaction that touches them. Pools are discovered dynamically in the stream and upserted into the `pools` table as they are observed. No upfront configuration — the `pools` table is a record of what yog-sothoth has seen, not a list of what it should watch.

### Current constraint — bounded allowlist

The public Solana RPC and the free Helius tier both cap transaction fetches at roughly 10 req/s. At peak DAMM v2 traffic (~105 qualified tx/s observed on mainnet), the indexer saturates by more than an order of magnitude, and `getTransaction` becomes the pipeline bottleneck (p99 fetch duration ~10s, accounting for 99% of `index_transaction` time).

Until an upgraded RPC path is available (Helius `transactionSubscribe` on the Developer plan, or the Startup Launchpad program), ingestion is bounded to a small **watched pools allowlist** stored in the `watched_pools` table. The protocol-centric architecture is preserved — the allowlist is applied as a filter in the ingestion pipeline, not as a return to static configuration. Lifting the constraint later is a matter of disabling the filter.

### The `watched_pools` table

| Column | Type | Purpose |
|---|---|---|
| `pool_address` | `TEXT PRIMARY KEY` | Solana pubkey of the pool |
| `protocol` | `TEXT NOT NULL` | Protocol identifier (`damm_v2`, etc.) |
| `active` | `BOOLEAN NOT NULL DEFAULT TRUE` | Whether the filter accepts events for this pool |
| `added_at` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` | When the pool was added to the allowlist |
| `note` | `TEXT` | Free-form annotation (selection rationale, edge-case marker, etc.) |

Deactivation uses the `active` flag rather than row deletion, to preserve history and allow reactivation without re-selection.

### Current selection

The allowlist was seeded from the 7-day activity distribution of `swap_events`. Pools were chosen to balance high-signal density (top of the distribution) with edge-case diversity (lower-activity pools for testing short-lived or thin-liquidity cases).

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

> **Note on observed activity patterns** — most pools in the selection exhibit burst behavior (high swap count over a short window, then quiescence). This is consistent with DAMM v2 being heavily used for memecoin launches. Longer-lived pools will be added as the dataset grows.

### Managing the allowlist

A seed script populates the 10 selection pools above in development environments — see *Getting started > Seed watched pools*.

For ad-hoc management:

```sql
-- Add a pool
INSERT INTO watched_pools (pool_address, protocol, note)
VALUES ('<pubkey>', 'damm_v2', 'manual selection: high TVL');

-- Deactivate without losing history
UPDATE watched_pools SET active = FALSE WHERE pool_address = '<pubkey>';

-- Reactivate
UPDATE watched_pools SET active = TRUE WHERE pool_address = '<pubkey>';

-- List currently active
SELECT pool_address, protocol, added_at, note
FROM watched_pools
WHERE active = TRUE
ORDER BY added_at DESC;
```

The filter is loaded at indexer startup. Hot reload will be added when user-managed watchlists land in **v0.3**.

### Removing the constraint

The allowlist is temporary. It will be lifted once one of the following is in place:

- Helius `transactionSubscribe` (Developer plan) — eliminates the HTTP fetch entirely, transactions arrive fully parsed in the WebSocket stream
- Helius Startup Launchpad — 8 months of Business tier free (LaserStream mainnet, 200 RPS)
- An equivalent gRPC provider (Shyft, Triton) with matching throughput

At that point, the filter is disabled (`active = TRUE` for all rows, or filter bypassed), and ingestion returns to full protocol-centric coverage.

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

### Seed watched pools

The indexer only processes pools present in the `watched_pools` allowlist (see *Pool observation model > Current constraint*). For development, a seed script populates the table with the 10 pools selected from recent mainnet activity:

```bash
# Requires the TimescaleDB container to be running (`docker compose up -d`)
docker compose exec -T timescaledb \
    psql -U yog -d yog_sothoth \
    < scripts/seed_watched_pools.sql
```

The script is idempotent (`ON CONFLICT DO NOTHING`) — safe to rerun without overwriting manual edits to `active` or `note`. It prints the current active allowlist at the end as a sanity check.

> Replace `timescaledb` with the actual service name declared in your `docker-compose.yml` if different.

### Debug helpers

```bash
# Inspect a specific signature (parse + classify, no persistence)
cargo run --bin debug_sig -- <signature>

# Run the indexer in release mode with logs persisted to disk
cargo run --release --bin yog-indexer 2>&1 | tee log/indexer-helius.log
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

### Observability

The indexer exposes Prometheus metrics on `:9000/metrics`:

- **Counters** — `raw_log_events_total`, `raw_log_events_rejected_total{filter, reason}`, `qualified_signatures_total`, `downstream_saturated_total`, `index_transaction_entered/exited_total{outcome}`, `instructions_indexed/skipped_total{instruction}`, `transactions_no_match_total`, `fetch_failures_total{reason}`, `fetch_not_found_total`, `watched_pool_filter_passed_total{pool_address}`, `watched_pool_filter_dropped_total`
- **Histograms** — `fetch_duration_seconds`, `persist_duration_seconds{kind}`, `index_transaction_duration_seconds{outcome}`
- **Gauges** — `watched_pools_active`

An `ExitGuard` RAII wrapper ensures every entry into `index_transaction` produces an exit counter and duration sample, even on error paths that do not explicitly tag an outcome.

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
- [x] Ingestion pipeline refactor — 3-stage split (`RpcListener` → `SignatureDispatcher` → `IndexerWorker`) with Prometheus instrumentation
- [ ] **Phase 1** — Indexer + `yog-core`: Solana WebSocket, DAMM v2 parser, AMM formulas, watched pools allowlist
- [ ] **Phase 2** — WASM integration + minimal Next.js dashboard
- [ ] **Phase 3** — Full REST API, time-series persistence, configurable alerts
- [ ] **Phase 4** — Production dashboard, CD, Clever Cloud deployment
- [ ] **Post-v0.1** — Upgrade to Helius `transactionSubscribe` (or equivalent), remove allowlist constraint, return to full protocol-centric ingestion

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