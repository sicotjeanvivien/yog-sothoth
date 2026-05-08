# yog-sothoth

![CI](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/ci.yml/badge.svg)

> Protocol-centric liquidity analytics engine for Meteora on Solana.

*Yog-Sothoth is the entity that sees everything simultaneously — past, present, future, all planes of existence at once. A fitting name for a tool that continuously observes the on-chain transaction stream of a protocol, reconstructs pool state over time, and detects patterns in DeFi liquidity flows.*

---

## What it is

yog-sothoth is a **protocol-centric** observer of Meteora's on-chain activity on Solana. It subscribes directly to Meteora program IDs, ingests every transaction that touches them, decodes the Anchor events emitted on-chain, and persists the reconstructed liquidity state in TimescaleDB. A separate HTTP API exposes the indexed data for dashboards, alerting, and analytics.

This is **not** a block explorer (≠ Solscan).
This is **not** an LP position tracker (≠ Ultra LP, TrackLP, MetLab).
It is a stream observer — pools are discovered dynamically, not configured upfront.

---

## Features

- **Real-time indexing** — WebSocket subscription per program, live event extraction
- **Anchor `event_cpi` decoding** — events read from on-chain emissions, not reconstructed from transfer instructions
- **AMM reconstruction** — price, reserves, slippage, imbalance computed from on-chain state
- **Time-series storage** — events stored in TimescaleDB hypertables with compression and retention policies
- **HTTP API** *(in progress)* — paginated REST endpoints over the indexed data
- **WASM in the browser** *(Phase 2)* — same Rust AMM formulas run client-side via WebAssembly
- **Configurable alerts** *(Phase 3)* — threshold-based notifications per pool
- **Push to dashboard** *(Phase 3)* — Server-Sent Events for low-latency updates without polling

---

## Architecture

### Two processes, one database

```
┌─────────────────────────────────────────────────────────┐
│                       Production                        │
│                                                         │
│  process 1 : yog-indexer (Rust)                         │
│  └── 3-stage pipeline (see below)                       │
│  └── Writes to TimescaleDB (role: yog_indexer)          │
│                                                         │
│  process 2 : yog-api (Rust, axum)                       │
│  └── HTTP server, JSON endpoints                        │
│  └── Reads from TimescaleDB (role: yog_api)             │
│                                                         │
│  process 3 : web (Next.js, scaffold)                    │
│  └── Dashboard UI, calls yog-api                        │
│                                                         │
│  TimescaleDB                                            │
│  └── sole communication channel between processes       │
└─────────────────────────────────────────────────────────┘
```

The indexer and the api communicate **only through the database** — no direct calls between them. The dashboard talks exclusively to the api over HTTP. This keeps each process small, independently deployable, and substitutable.

### Indexer — three-stage pipeline

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

**`RpcListener`** owns the WebSocket connection, handles reconnection with exponential backoff, and forwards raw log events downstream.

**`SignatureDispatcher`** applies a chain of filters that turn raw log events into qualified `(protocol, signature)` pairs — drops failed transactions, transactions that don't actually invoke the watched protocol, and (temporarily) transactions outside the watched-pool allowlist.

**`IndexerWorker`** consumes qualified signatures and drives `IndexerService` with bounded concurrency. The cap is `MAX_CONCURRENT_INDEX_TASKS = 15`, calibrated against the Helius free tier (10 req/s) with headroom. Per-signature failures are logged and counted but never stop the pipeline; loop-level failures (closed channels, exhausted semaphore, panics) bubble up to the daemon and trigger graceful shutdown via a shared `CancellationToken`.

This **skip-and-log** semantic also applies inside `IndexerService`: when a transaction yields multiple events, a failure persisting one event never aborts the others.

### API — axum HTTP server

The api is a separate binary built on [axum](https://docs.rs/axum/) (0.8). It exposes JSON endpoints over the indexed data, with cursor-based pagination by default. A single `AppState` instance — built once at startup and clonable for free thanks to `Arc`-wrapped fields — is shared across handlers via axum's `State` extractor.

Currently exposed:

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness probe (200 OK, no DB roundtrip) |
| `GET` | `/api/pools` | Paginated list of discovered pools (cursor-based, `limit` 1–200, default 50) |

The cursor is opaque to clients: it's a base64(url-safe, no-pad) encoding of a typed `PoolCursor` produced by the previous response. Clients pass it back unchanged via `?cursor=...`. End of pagination is signalled by `next_cursor: null`.

CORS is currently permissive for development; security headers (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`) are applied as router-level middleware.

### Extraction — Anchor `event_cpi` decoding

Meteora programs emit their events via Anchor's `emit_cpi!` mechanism: a self-CPI to an `event_authority` PDA, with a stable wire format:

```
[8 bytes EVENT_IX_TAG][8 bytes event discriminator][borsh payload]
```

where `EVENT_IX_TAG = sha256("anchor:event")[..8]` is the fixed prefix Anchor injects.

The indexer decodes these emissions directly rather than reconstructing events from `transferChecked` SPL instructions. This is the source-of-truth approach: events are read from what the program *says it did*, not inferred from the side effects.

Concrete benefits:

- **Stability** — no dependency on per-instruction account ordering, which differs across `Swap` / `AddLiquidity` / `RemoveLiquidity` variants
- **Coverage** — `EvtClaimPositionFee` and `EvtClaimReward` are captured natively, where `transferChecked` reconstruction couldn't distinguish them cleanly
- **Extensibility** — adding a new event is one wire mirror + one translator arm, no instruction-multiplexing logic

The full pipeline (generic Anchor decoder → DAMM v2 wire events → wire-to-domain translator) is documented in [`crates/README.md`](./crates/README.md#anchor-event_cpi-extraction-pipeline).

---

## Workspace

The Rust workspace is split into five library crates and two binaries, each with a single responsibility:

| Crate | Type | Role |
|---|---|---|
| `yog-core` | lib | Domain types, repository traits, AMM formulas, Anchor `event_cpi` decoding. Wasm-compatible by design — never depends on I/O. |
| `yog-persistence` | lib | Postgres adapter. Concrete implementations of the repository traits, sqlx queries, migrations. |
| `yog-bootstrap` | lib | Shared startup utilities: env-var helpers, `SecretUrl`, `ConfigError`, `init_rustls`, `init_tracing`. Consumed by both binaries. |
| `yog-indexer` | bin | Long-running ingestion pipeline. Owns the `RpcListener` / `Dispatcher` / `Worker` stack. |
| `yog-api` | bin | HTTP server (axum). Reads from the database and serves JSON. |
| `yog-wasm` | bin | Browser-facing wrapper around `yog-core`. Currently a scaffold (see *Feature flags*). |

```
yog-sothoth/
├── crates/
│   ├── core/
│   ├── persistence/
│   ├── bootstrap/
│   ├── indexer/
│   ├── api/
│   └── wasm/
└── web/             ← Next.js dashboard (scaffold minimal — Phase 2)
```

### Dual compilation target — `yog-core`

The `core` crate compiles to two targets from the same source:

- `cargo build` → native binary used by the indexer and the api
- `wasm-pack build` → WASM module loaded by Next.js

Price and slippage calculations are **identical** between backend and frontend — no possible divergence. The WASM target is currently a scaffold; making it functional requires gating Solana-only modules behind a feature flag. Scheduled for Phase 2 — see [`crates/README.md`](./crates/README.md#wasm-yog-wasm).

---

## Database roles

The two processes (`yog-indexer` and `yog-api`) connect to Postgres under **distinct roles** with least-privilege grants:

| Role | Permissions | Used by |
|---|---|---|
| `yog_indexer` | `SELECT, INSERT, UPDATE` on event tables; `SELECT` on `watched_pools` | indexer process |
| `yog_api` | `SELECT` on event tables and `watched_pools` (will gain `INSERT/UPDATE` on user-facing tables in v0.3) | api process |
| admin (provisioning role) | full DDL — owns the schema, runs migrations, used by `cargo sqlx prepare` | tooling only, never a running service |

The split is enforced at the database level. A bug or compromise in the api cannot corrupt event data — Postgres rejects the operation before the SQL is ever sent. Roles and grants are provisioned once per database via [`crates/persistence/setup_roles.sql`](./crates/persistence/setup_roles.sql).

Future tables that need write access from the api (users, alert subscriptions in v0.3) will require explicit `GRANT` per migration. Default privileges grant `SELECT` to both roles automatically; `INSERT/UPDATE` are intentionally not in defaults to force a conscious decision per table.

---

## Stack

| Layer | Technology | Role |
|---|---|---|
| Indexer | Rust, Tokio | WebSocket listener, event decoding, persistence |
| HTTP API | Rust, axum, tower | JSON endpoints, cursor-based pagination |
| AMM engine | Rust → WASM | Shared `yog-core` crate compiled for both native and browser |
| Frontend | Next.js / TypeScript *(scaffold minimal)* | Dashboard UI — Phase 2 onwards |
| Database | TimescaleDB | Time-series storage, hypertables, compression, retention |
| Transport | WebSocket (in), HTTP/SSE (out) | Solana RPC inbound; HTTP REST today, SSE planned for Phase 2 push |

---

## Pool observation model

### Long-term target — protocol-centric

The target design is **protocol-centric**: the indexer subscribes directly to Meteora program IDs and ingests every transaction that touches them. Pools are discovered dynamically in the stream and upserted into the `pools` table as they are observed. No upfront configuration — the `pools` table is a record of what yog-sothoth has seen, not a list of what it should watch.

### Current constraint — bounded allowlist

The public Solana RPC and the free Helius tier both cap transaction fetches at roughly 10 req/s. At peak DAMM v2 traffic (~105 qualified tx/s observed on mainnet), the indexer saturates by more than an order of magnitude, and `getTransaction` becomes the pipeline bottleneck (p99 fetch duration ~10s, accounting for 99% of `index_transaction` time).

Until an upgraded RPC path is available (Helius `transactionSubscribe` on the Developer plan, or the Startup Launchpad program), ingestion is bounded to a small **watched pools allowlist** stored in the `watched_pools` table. The protocol-centric architecture is preserved — the allowlist is applied as a filter inside the dispatcher's filter chain, not as a return to static configuration. Lifting the constraint later is a matter of disabling the filter.

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

| Priority | Protocol | Status | Model |
|---|---|---|---|
| **Phase 1** | Meteora DAMM v2 | **Active** — Cercle 1 events end-to-end | x·y=k + dynamic fees + NFT positions |
| Phase 2 | Meteora DLMM | Stub | Bin-based liquidity, volatility fees |
| Phase 2 | Meteora DAMM v1 | Stub | x·y=k + dual-yield (lending) |
| Phase 3+ | DAMM v1 Farm, Stake2Earn, LST, Multi-Token | Not started | — |

For DAMM v2, "Cercle 1" covers `EvtSwap2`, `EvtLiquidityChange`, `EvtClaimPositionFee`, `EvtClaimReward` — the events that drive the LP-observation model. Cercles 2 (position lifecycle, pool config) and 3 (admin) are scoped but not yet wired.

---

## Hosting

Production deployment targets **Scaleway** (Paris region):

- **Compute** — single instance (DEV1-M class) running the indexer, the api, and Caddy as reverse proxy, all containerized via Docker Compose
- **Database** — Scaleway Managed PostgreSQL with the TimescaleDB extension activated
- **Backups** — Scaleway Object Storage (One Zone IA), daily `pg_dump` uploaded by cron, 30-day rolling retention
- **TLS** — Caddy handles certificates automatically via Let's Encrypt; processes themselves listen plain HTTP on internal addresses
- **Monitoring** — Healthchecks.io for the indexer heartbeat, Uptime Kuma for HTTP probes

The full hosting layout, Compose files, and procedure are documented separately in [`Fiche_d_hébergement___Scaleway_full-stack.md`](./Fiche_d_hébergement___Scaleway_full-stack.md). Approximate monthly cost: ~20 € HT.

---

## Getting started

### Prerequisites

- Rust (stable) — [rustup.rs](https://rustup.rs)
- `wasm-pack` — [rustwasm.github.io/wasm-pack](https://rustwasm.github.io/wasm-pack)
- Node.js LTS — via [nvm](https://github.com/nvm-sh/nvm)
- Docker (for TimescaleDB)
- The sqlx CLI for database setup: `cargo install sqlx-cli --no-default-features --features postgres`

### Environment variables

Copy `.env.example` to `.env` and fill in:

```env
# Three roles, three connection strings
DATABASE_URL_INDEXER=postgresql://yog_indexer:CHANGE_ME@localhost:5433/yog_sothoth
DATABASE_URL_API=postgresql://yog_api:CHANGE_ME@localhost:5433/yog_sothoth
DATABASE_URL_ADMIN=postgresql://postgres:CHANGE_ME@localhost:5433/yog_sothoth

SQLX_OFFLINE=true

# API server bind address (host:port)
API_BIND_ADDR=127.0.0.1:3000

# Solana RPC
SOLANA_RPC_WS=wss://api.mainnet-beta.solana.com
SOLANA_RPC_HTTP=https://api.mainnet-beta.solana.com

# Indexer behavior
RPC_WORKER_MAX_RETRIES=10
MODE_PROTOCOL_CENTRIC=true

# Observability
RUST_LOG=yog_indexer=debug,yog_api=debug,yog_core=debug,yog_persistence=debug,sqlx=warn
LOG_FORMAT=text
```

`DATABASE_URL_INDEXER` and `DATABASE_URL_API` are read by the indexer and the api respectively at startup. `DATABASE_URL_ADMIN` is used only by tooling (`sqlx migrate run`, `cargo sqlx prepare`).

### Run locally

```bash
# Clone
git clone https://github.com/sicotjeanvivien/yog-sothoth.git
cd yog-sothoth

# Start TimescaleDB
docker compose up -d

# Provision the Postgres roles (one-time, runs as the admin user)
docker compose exec -T timescaledb \
    psql -U postgres -d yog_sothoth \
    < crates/persistence/setup_roles.sql

# Apply migrations (uses DATABASE_URL_ADMIN)
sqlx migrate run --source crates/persistence/migrations \
                 --database-url "$DATABASE_URL_ADMIN"

# Build the workspace
cargo build

# Seed the watched-pools allowlist
docker compose exec -T timescaledb \
    psql -U yog_indexer -d yog_sothoth \
    < scripts/seed_watched_pools.sql

# Run the indexer (connects as yog_indexer)
cargo run -p yog-indexer

# Run the api (connects as yog_api) — separate terminal
cargo run -p yog-api

# Hit the api
curl http://127.0.0.1:3000/healthz
curl http://127.0.0.1:3000/api/pools | jq

# Build the WASM module (scaffold — not yet wired to Next.js)
wasm-pack build crates/wasm --target web

# Install and run Next.js (scaffold minimal — Phase 2 onwards)
cd web
npm install
npm run dev

# Tear down the database (with volume)
docker compose down -v
```

### Debug helpers

```bash
# Inspect a specific signature (fetch + decode + classify, no persistence)
cargo run --bin debug_sig -- <signature>

# Run the indexer in release mode with logs persisted to disk
cargo run --release -p yog-indexer 2>&1 | tee log/indexer.log
```

---

## Development

### Local checks before pushing

The CI mirrors these three commands. Run them locally to catch issues before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features
SQLX_OFFLINE=true cargo test --workspace --all-features
```

### Modifying SQL queries

SQLx compile-time query verification runs in offline mode via `SQLX_OFFLINE=true`, using the committed `.sqlx/` cache. **When you modify an `sqlx::query!` call, regenerate the cache** before committing — using the admin role, since runtime roles don't have introspection privileges on every table:

```bash
DATABASE_URL="$DATABASE_URL_ADMIN" \
SQLX_OFFLINE=false \
cargo sqlx prepare --workspace
```

Commit the modified files under `.sqlx/` together with the query change.

### Feature flags

`yog-core` exposes two features to support its dual compilation target:

| Feature | Default | Purpose |
|---|---|---|
| `solana` | ✅ | Pulls in `solana-pubkey`, `solana-transaction-status`. Required by `yog-indexer` and `yog-api`. |
| `wasm` | — | Reserved for the browser build. **Not yet functional.** |

The `wasm` feature is currently a placeholder. Activating it will require conditional compilation (`#[cfg(feature = "solana")]`) on the `amm` and `protocols` modules, plus abstracting `Pubkey` behind a neutral type alias. Scheduled for **Phase 2**.

### Observability

The indexer exposes Prometheus metrics on `:9000/metrics`. The api will expose its own metrics through axum middleware on its HTTP server when needed (different histograms and labels — no symmetry with the indexer worth sharing).

The most important indexer metric families:

- **Pipeline counters** — `raw_log_events_total`, `raw_log_events_rejected_total{filter, reason}`, `qualified_signatures_total`, `downstream_saturated_total`
- **Service counters** — `index_transaction_entered/exited_total{outcome}`, `events_indexed_total{event_kind}`, `transactions_no_match_total`, `unknown_event_total{discriminator}`, `extraction_failure_total{kind}`
- **Persistence** — `persist_failures_total{event_kind}`, `fetch_failures_total{reason}`, `fetch_not_found_total`
- **Allowlist filter** — `watched_pool_filter_passed_total{pool_address}`, `watched_pool_filter_dropped_total`
- **Histograms** — `fetch_duration_seconds`, `persist_duration_seconds{kind}`, `index_transaction_duration_seconds{outcome}`
- **Gauges** — `watched_pools_active`

An `ExitGuard` RAII wrapper ensures every entry into `index_transaction` produces an exit counter and duration sample, even on error paths that do not explicitly tag an outcome.

### Continuous integration

GitHub Actions runs on every push and pull request to `main`:

- **Format** — `cargo fmt --all -- --check` (strict)
- **Lint** — `cargo clippy --workspace --all-targets --all-features` (warnings non-blocking during initial development)
- **Tests** — `cargo test --workspace --all-features` (strict)

WASM builds are not part of CI yet — see *Feature flags* above.

### Lint policy

Clippy warnings are currently **non-blocking** in CI. The project is in an early architectural phase with deliberate scaffolding (e.g. unused fields anticipating future protocol support). `-D warnings` will be reactivated once the core architecture stabilizes (target: end of Phase 1).

---

## Roadmap

### v0.1 — Indexer + API + dashboard MVP *(in progress, target: end of June 2026)*

- [x] Project setup — Rust workspace, TimescaleDB, Next.js scaffold
- [x] CI pipeline — GitHub Actions (fmt, clippy, tests) with SQLx offline verification
- [x] Three-stage ingestion pipeline (`RpcListener` → `SignatureDispatcher` → `IndexerWorker`) with Prometheus instrumentation
- [x] DAMM v2 indexer — Anchor `event_cpi` decoding, Cercle 1 events end-to-end (Swap, Liquidity, ClaimFee, ClaimReward)
- [x] Workspace refondation — `core` / `persistence` / `bootstrap` separation, two-role Postgres setup
- [🟡] HTTP API on axum — `GET /healthz` and `GET /api/pools` (paginated) live; remaining endpoints (swaps, liquidity events, per-pool detail) pending
- [ ] Minimal Next.js dashboard consuming `/api/pools`
- [ ] Configurable alerts (threshold-based)
- [ ] Production dashboard, CD, Scaleway deployment

### v0.2 — Signal Engine *(target: end of September 2026)*

Pattern detection on accumulated event data: TVL drain, fee yield spike, imbalance alerts, price impact creep. New `signals` crate, multi-channel alert delivery (webhook / email / Telegram). Server-Sent Events on the api for low-latency push to the dashboard.

### v0.3 — Auth and per-user pool watchlists *(target: end of November 2026)*

Multi-channel authentication (email, OAuth, Solana wallet), per-user pool watchlists, tier infrastructure with placeholder quotas. The `yog_api` Postgres role gains `INSERT/UPDATE` on user-facing tables.

### v0.4 — Monetization *(target: February-March 2027)*

Stripe billing, public pricing tiers, API keys with rate limiting, enterprise / white-label offering.

### v0.5 — Extended Meteora coverage *(unscheduled)*

DLMM, DAMM v1, DAMM v1 Farm, Stake2Earn, LST, Multi-Token. Extends the existing extraction pipeline; no architectural changes expected.

### Post-Phase 1 RPC upgrade

Move to Helius `transactionSubscribe` (Developer plan) or an equivalent gRPC provider (Shyft, Triton). Removes the watched-pool allowlist constraint and returns ingestion to fully protocol-centric coverage.

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
2. If you modified an `sqlx::query!` call, regenerate `.sqlx/` with the admin role (see *Modifying SQL queries*)
3. Open your PR against `main` — GitHub Actions will run automatically

---

## License

[MIT](./LICENSE)