# yog-sothoth

[![crates](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/crates.yml/badge.svg)](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/crates.yml)
[![web-quality](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/web-quality.yml/badge.svg)](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/web-quality.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

> Protocol-centric liquidity analytics engine for Meteora on Solana.

*Yog-Sothoth is the entity that sees everything simultaneously — past, present, future, all planes of existence at once. A fitting name for a tool that continuously observes the on-chain transaction stream of a DeFi protocol, reconstructs pool state in real time, and surfaces patterns in liquidity flows.*

---

## What is yog-sothoth?

yog-sothoth is a **protocol-centric** observer of Meteora's on-chain activity on Solana. It subscribes directly to Meteora program IDs, ingests every transaction that touches them, decodes the Anchor events emitted on-chain, and persists the reconstructed liquidity state in TimescaleDB. A dedicated enrichment service complements raw on-chain data with token metadata and USD prices, a **signal engine** runs pattern detectors over the accumulated data and emits typed alerts, and an HTTP API exposes everything — including a live signal feed over Server-Sent Events — to the Next.js dashboard and any other consumer.

This is **not** a block explorer (≠ Solscan).
This is **not** an LP position tracker (≠ Ultra LP, TrackLP, MetLab).
It is a **stream observer** — pools are discovered dynamically as transactions flow in, not configured upfront. The `pools` table is a record of what yog-sothoth has *seen*, not a list of what it should *watch*.

---

## Features

- **Real-time indexing** — WebSocket subscription per Meteora program, live Anchor event extraction
- **Anchor `event_cpi` decoding** — events read from on-chain emissions, not reconstructed from transfer instructions
- **AMM state reconstruction** — price, reserves, slippage, imbalance computed from the event stream
- **Token enrichment** — symbol / name / decimals / logo via Helius DAS, USD prices via Jupiter Price V3
- **Signal detection** — batch detectors evaluated over the indexed data (swap-flow imbalance, spot-vs-oracle price deviation), Warning/Critical severities, cooldown-based deduplication
- **Live signal feed** — `GET /api/signals` (paginated) plus an SSE stream, consumed by a dashboard feed page that updates as signals fire
- **Time-series storage** — TimescaleDB hypertables with compression, retention policies, and continuous aggregates
- **HTTP API** — JSON endpoints with cursor-based pagination and SSE streaming, served by an axum-based server
- **Per-process database roles** — least-privilege Postgres roles, one per binary, enforced at the database level
- **Docker stack** — full local development environment via `docker compose`, profile-driven
- **WASM in the browser** *(deferred)* — same Rust AMM formulas run client-side via WebAssembly; reassessed at v0.2

---

## How it works (high-level)

Five processes share a single Postgres database — no direct calls between them, all coordination happens through the schema:

```
                ┌──────────────────────────────────────────────────────┐
                │                 TimescaleDB (Postgres)               │
                │  events · pools · token_metadata · prices · signals  │
                └─────┬───────────┬────────────┬────────────────┬──────┘
                      │ writes    │ writes     │ writes         │ reads
                ┌─────┴────┐ ┌────┴─────┐ ┌────┴──────┐ ┌───────┴──────┐
                │ indexer  │ │ context  │ │  signals  │ │     api      │
                │  (Rust)  │ │  (Rust)  │ │  (Rust)   │ │ (Rust, axum) │
                └─────┬────┘ └────┬─────┘ └───────────┘ └───────┬──────┘
                      │ WebSocket │ HTTP                        │ HTTP + SSE
                      ▼           ▼                             ▼
                 Solana RPC   Helius DAS               web (Next.js) · browser
                 (Helius)     Jupiter Price V3
```

- **`indexer`** subscribes to Meteora programs, decodes Anchor events, persists the reconstructed state. Three-stage pipeline with bounded concurrency and Prometheus metrics.
- **`context`** enriches the raw mint addresses recorded by the indexer with token metadata (Helius DAS) and USD prices (Jupiter Price V3), and resolves pool properties (mints, fee config) from on-chain accounts. Independent worker loops with configurable intervals.
- **`signals`** is a batch detector engine: each detector polls the accumulated data at its own cadence, stateless between ticks — the database carries the state — and emits typed signals with a severity into the `signals` table. A per-`(detector, pool)` cooldown prevents re-alerting, except on severity escalation.
- **`api`** exposes the indexed, enriched, and detected data over HTTP. Cursor-based pagination, RFC 9457 errors, security headers as router-level middleware. It is also the single egress for signals: a paginated collection endpoint plus an SSE stream fed by an internal poller that broadcasts new signals to connected clients.
- **`web`** is a Next.js dashboard. Server Components render the initial data from the API; the browser then talks to the API directly (CORS-locked) — there is no BFF layer.

Migrations are applied by a separate one-shot binary (`yog-migrate`) that runs once per deployment under its own DDL role. Runtime services never have schema-modification privileges — each of the five processes connects under its own least-privilege Postgres role.

For the full ingestion pipeline, the Anchor decoding mechanism, the database role split, and the workspace layout, see **[`crates/README.md`](./crates/README.md)**. For the dashboard architecture, see **[`web/README.md`](./web/README.md)**.

---

## Pool observation model

The long-term design is **protocol-centric**: the indexer subscribes to Meteora program IDs and ingests every transaction that touches them, discovering pools as they appear in the stream.

In the current phase, ingestion is bounded by a **temporary allowlist** stored in the `watched_pools` table — the public Solana RPC and the free Helius tier both cap transaction fetches at roughly 10 req/s, and peak DAMM v2 traffic saturates that by more than an order of magnitude. The allowlist is applied as a filter inside the dispatcher's filter chain, not as a return to static configuration: lifting the constraint is a matter of disabling the filter.

The allowlist will be lifted once an upgraded RPC path is in place — Helius `transactionSubscribe`, or a managed Yellowstone gRPC (Geyser) stream (Shyft, Triton, Helius LaserStream…). Only the subscription layer of the indexer changes; the extraction → persistence pipeline stays as is.

For administering the allowlist (schema, seed scripts, SQL helpers), see **[`crates/persistence/README.md`](./crates/persistence/README.md)**.

---

## Supported protocols

| Protocol | Status | Model |
|---|---|---|
| Meteora DAMM v2 | **Active** — 11 event kinds end-to-end (circles 1–3) | x·y=k + dynamic fees + NFT positions |
| Meteora DLMM | Stub | Bin-based liquidity, volatility fees |
| Meteora DAMM v1 | Stub | x·y=k + dual-yield (lending) |
| DAMM v1 Farm, Stake2Earn, LST, Multi-Token | Not started | — |

For DAMM v2, "circle 1" covers `EvtSwap2`, `EvtLiquidityChange`, `EvtClaimPositionFee`, `EvtClaimReward` — the events that drive the LP-observation model. Circle 2 (position lifecycle — `EvtCreatePosition`, `EvtClosePosition`, `EvtLockPosition`, `EvtPermanentLockPosition`) and circle 3 (pool config / admin — `EvtInitializePool`, `EvtSetPoolStatus`, `EvtUpdatePoolFees`) are wired end-to-end as well: extracted, persisted to their own per-kind tables, and covered by fixture tests. Each lands in `meteora_damm_v2_<kind>_events`; cross-protocol VIEWs expose only the four circle-1 concepts.

---

## Stack

| Layer | Technology |
|---|---|
| Indexer, enrichment, signals, API | Rust 1.95, Tokio, axum, sqlx |
| Database | TimescaleDB on PostgreSQL 16 |
| Frontend | Next.js 16, TypeScript, Tailwind v4, next-intl |
| RPC providers | Helius (WebSocket + HTTP + DAS), Jupiter Price V3 |
| Container runtime | Docker Compose (5 backend images + 1 frontend image) |
| Reverse proxy | Caddy (automatic TLS via Let's Encrypt) |
| Observability | Prometheus, tracing |
| CI | GitHub Actions (cargo check / fmt / clippy / test / audit, sqlx offline check) |

---

## Getting started

The fastest path is via Docker. The repo ships a complete `docker-compose.yml` with profiles, so you can spin up exactly what you need.

```bash
# Clone
git clone https://github.com/sicotjeanvivien/yog-sothoth.git
cd yog-sothoth

# Copy the env template and fill in CHANGE_ME values (DB passwords, Helius key, Jupiter key)
cp .env.example .env

# Start Postgres only (smallest footprint, useful when running native cargo run alongside)
docker compose up -d

# Provision the database roles (one-time, as superuser)
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f crates/persistence/setup_roles.sql

# Bring up the full backend stack (postgres + migrate + indexer + api + context + signals)
docker compose --profile backend up -d --build

# Or the full stack including the web dashboard
docker compose --profile full up -d --build
```

The API is reachable on `http://localhost:5000` and the web dashboard on `http://localhost:3000`. Each daemon exposes Prometheus metrics on the host: indexer on `127.0.0.1:9000/metrics`, context on `:9001`, signals on `:9002`.

For the native development workflow (running services via `cargo run` against a dockerised Postgres), the migration toolchain, and the SQL conventions, see **[`crates/README.md`](./crates/README.md)**. For the frontend setup, see **[`web/README.md`](./web/README.md)**.

---

## Roadmap

### v0.1 — Analyzer + Signal Engine *(in progress)*

Originally two releases, merged in June 2026: an on-chain analytics tool without detectors is an event viewer, not a product — no public release until there are signals to offer. The internal split is kept to preserve the build order.

**v0.1.0 — Analyzer** ✅ *(complete — internal POC, no public release)*

- [x] Rust workspace — `core` / `persistence` / `bootstrap` / `indexer` / `api` / `context` / `wasm`
- [x] Three-stage ingestion pipeline (`RpcListener` → `SignatureDispatcher` → `IndexerWorker`) with Prometheus instrumentation
- [x] DAMM v2 decoding — Anchor `event_cpi`, 11 event kinds end-to-end (swap/liquidity/claims, position lifecycle, pool config & admin)
- [x] Token enrichment daemon — metadata via Helius DAS, USD prices via Jupiter Price V3, pool account resolution (mints, fee config)
- [x] HTTP API on axum — pools (list, detail, top-N, history), tokens, global stats
- [x] Realised-fee analytics — continuous aggregates, USD valuation views, fee charts on the pool page
- [x] Next.js dashboard — overview (KPIs + top pools), pools list, pool detail with charts
- [x] Least-privilege Postgres model — one role per process, forward-only migrations via `yog-migrate`
- [x] Full Docker stack and CI (check / fmt / clippy / test / audit, sqlx offline check)

**v0.1.1 — Signal Engine + release prep** *(in progress, blocks the public deployment)*

- [x] `signals` process — batch detector engine, per-detector cadence, cooldown deduplication, Prometheus metrics
- [x] First two detectors — swap-flow imbalance, spot-vs-oracle price deviation (with freshness guards)
- [x] `signals` hypertable + `yog_signals` role
- [x] `GET /api/signals` (cursor pagination) + `GET /api/signals/stream` (SSE)
- [x] Live signals feed page in the dashboard
- [ ] Signals page UX pass (hierarchy, severity filter, pagination)
- [ ] Next detectors — fee yield spike, TVL drain
- [ ] Telegram operator channel
- [ ] Pre-release audit (security, conventions) and legal pages (privacy, terms)
- [ ] Scaleway deployment *(scheduled to start early August 2026)*

### v0.2 — Auth and per-user pool watchlists

Multi-channel authentication (email, OAuth, Solana wallet), per-user pool watchlists, tier infrastructure with placeholder quotas. The `yog_api` Postgres role gains `INSERT/UPDATE` on user-facing tables. WASM activation reassessed at this point.

### Later *(unscheduled)*

- **Monetization** — Stripe billing, public pricing tiers, API keys with rate limiting.
- **Extended Meteora coverage** — DLMM, DAMM v1, DAMM v1 Farm, Stake2Earn, LST, Multi-Token. Extends the existing extraction pipeline; no architectural changes expected.
- **RPC upgrade** — Helius `transactionSubscribe` or a managed Yellowstone gRPC (Geyser) provider. Removes the watched-pool allowlist constraint and returns ingestion to fully protocol-centric coverage.

---

## Hosting

Production deployment targets **Scaleway** in the Paris region — a single instance running the five backend containers (`yog-migrate`, `yog-indexer`, `yog-api`, `yog-context`, `yog-signals`) plus the frontend and Caddy as reverse proxy, with a Managed PostgreSQL instance carrying the TimescaleDB extension and Object Storage for daily `pg_dump` backups. Approximate monthly cost: **~20 € HT**.

---

## Contributing

Contributions are welcome. Open an issue first to discuss what you'd like to change.

- For Rust conventions (formatting, clippy policy, sqlx offline workflow), see **[`crates/README.md`](./crates/README.md)**.
- For TypeScript conventions (lint, typecheck, test), see **[`web/README.md`](./web/README.md)**.

Before opening a PR, run the relevant local checks (mirrored by CI) and make sure your changes ship with their tests where applicable.

---

## License

[MIT](./LICENSE)
