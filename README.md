# yog-sothoth

[![crates](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/crates.yml/badge.svg)](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/crates.yml)
[![web-quality](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/web-quality.yml/badge.svg)](https://github.com/sicotjeanvivien/yog-sothoth/actions/workflows/web-quality.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

> Protocol-centric liquidity analytics engine for Meteora on Solana.

*Yog-Sothoth is the entity that sees everything simultaneously — past, present, future, all planes of existence at once. A fitting name for a tool that continuously observes the on-chain transaction stream of a DeFi protocol, reconstructs pool state in real time, and surfaces patterns in liquidity flows.*

---

## What is yog-sothoth?

yog-sothoth is a **protocol-centric** observer of Meteora's on-chain activity on Solana. It subscribes directly to Meteora program IDs, ingests every transaction that touches them, decodes the Anchor events emitted on-chain, and persists the reconstructed liquidity state in TimescaleDB. A dedicated enrichment service complements raw on-chain data with token metadata and USD prices, and an HTTP API exposes the indexed data to dashboards, alerting tools, and analytics consumers.

This is **not** a block explorer (≠ Solscan).
This is **not** an LP position tracker (≠ Ultra LP, TrackLP, MetLab).
It is a **stream observer** — pools are discovered dynamically as transactions flow in, not configured upfront. The `pools` table is a record of what yog-sothoth has *seen*, not a list of what it should *watch*.

---

## Features

- **Real-time indexing** — WebSocket subscription per Meteora program, live Anchor event extraction
- **Anchor `event_cpi` decoding** — events read from on-chain emissions, not reconstructed from transfer instructions
- **AMM state reconstruction** — price, reserves, slippage, imbalance computed from the event stream
- **Token enrichment** — symbol / name / decimals / logo via Helius DAS, USD prices via Jupiter Price V3
- **Time-series storage** — TimescaleDB hypertables with compression and retention policies
- **HTTP API** — JSON endpoints with cursor-based pagination, served by an axum-based server
- **Per-process database roles** — least-privilege Postgres roles, one per binary, enforced at the database level
- **Docker stack** — full local development environment via `docker compose`, profile-driven
- **Configurable alerts** *(v0.2)* — threshold-based notifications per pool, multi-channel delivery
- **WASM in the browser** *(v0.3 — deferred)* — same Rust AMM formulas run client-side via WebAssembly

---

## How it works (high-level)

Four processes share a single Postgres database — no direct calls between them, all coordination happens through the schema:

```
                    ┌─────────────────────────────────────────────┐
                    │              TimescaleDB (Postgres)         │
                    │  events · pools · token_metadata · prices   │
                    └─┬───────────┬───────────────┬───────────┬───┘
                      │ writes    │ writes        │ reads     │ reads
                      │           │               │           │
              ┌───────┴────┐ ┌────┴───────┐ ┌─────┴───┐ ┌─────┴─────┐
              │  indexer   │ │  context   │ │   api   │ │ web (BFF) │
              │  (Rust)    │ │  (Rust)    │ │ (Rust,  │ │ (Next.js) │
              │            │ │            │ │  axum)  │ │           │
              └──────┬─────┘ └─────┬──────┘ └─────────┘ └─────┬─────┘
                     │ WebSocket   │ HTTP                      │ HTTP
                     ▼             ▼                           ▼
              Solana RPC     Helius DAS                   Browser
              (Helius)       Jupiter Price V3
```

- **`indexer`** subscribes to Meteora programs, decodes Anchor events, persists the reconstructed state. Three-stage pipeline with bounded concurrency and Prometheus metrics.
- **`context`** enriches the raw mint addresses recorded by the indexer with token metadata (Helius DAS) and USD prices (Jupiter Price V3). Two independent worker loops with configurable intervals.
- **`api`** exposes the indexed and enriched data over HTTP. Cursor-based pagination, security headers as router-level middleware, single `AppState` shared across handlers.
- **`web`** is a Next.js dashboard with a thin BFF layer that proxies the API for the browser.

Migrations are applied by a separate one-shot binary (`yog-migrate`) that runs once per deployment under its own DDL role. Runtime services never have schema-modification privileges.

For the full ingestion pipeline, the Anchor decoding mechanism, the database role split, and the workspace layout, see **[`crates/README.md`](./crates/README.md)**. For the dashboard architecture, see **[`web/README.md`](./web/README.md)**.

---

## Pool observation model

The long-term design is **protocol-centric**: the indexer subscribes to Meteora program IDs and ingests every transaction that touches them, discovering pools as they appear in the stream.

In the current phase, ingestion is bounded by a **temporary allowlist** stored in the `watched_pools` table — the public Solana RPC and the free Helius tier both cap transaction fetches at roughly 10 req/s, and peak DAMM v2 traffic saturates that by more than an order of magnitude. The allowlist is applied as a filter inside the dispatcher's filter chain, not as a return to static configuration: lifting the constraint is a matter of disabling the filter.

The allowlist will be lifted once an upgraded RPC path is in place — Helius `transactionSubscribe` (Developer plan), Helius Startup Launchpad, or an equivalent gRPC provider (Shyft, Triton).

For administering the allowlist (schema, seed scripts, SQL helpers), see **[`crates/persistence/README.md`](./crates/persistence/README.md)**.

---

## Supported protocols

| Priority | Protocol | Status | Model |
|---|---|---|---|
| **v0.1** | Meteora DAMM v2 | **Active** — circle 1 events end-to-end | x·y=k + dynamic fees + NFT positions |
| v0.5 | Meteora DLMM | Stub | Bin-based liquidity, volatility fees |
| v0.5 | Meteora DAMM v1 | Stub | x·y=k + dual-yield (lending) |
| v0.5+ | DAMM v1 Farm, Stake2Earn, LST, Multi-Token | Not started | — |

For DAMM v2, "circle 1" covers `EvtSwap2`, `EvtLiquidityChange`, `EvtClaimPositionFee`, `EvtClaimReward` — the events that drive the LP-observation model. Circles 2 (position lifecycle, pool config) and 3 (admin) are scoped but not yet wired.

---

## Stack

| Layer | Technology |
|---|---|
| Indexer, enrichment, API | Rust 1.86, Tokio, axum, sqlx |
| Database | TimescaleDB on PostgreSQL 16 |
| Frontend | Next.js 16, TypeScript, Tailwind v4, next-intl |
| RPC providers | Helius (WebSocket + HTTP + DAS), Jupiter Price V3 |
| Container runtime | Docker Compose (4 backend images + 1 frontend image) |
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

# Bring up the full backend stack (postgres + migrate + indexer + api + context)
docker compose --profile backend up -d --build

# Or the full stack including the web dashboard
docker compose --profile full up -d --build
```

The API is reachable on `http://localhost:5000`, the web dashboard on `http://localhost:3000`, and the indexer's Prometheus `/metrics` on `http://127.0.0.1:9000/metrics`.

For the native development workflow (running services via `cargo run` against a dockerised Postgres), the migration toolchain, and the SQL conventions, see **[`crates/README.md`](./crates/README.md)**. For the frontend setup (`npm install`, dev server, BFF routes), see **[`web/README.md`](./web/README.md)**.

---

## Roadmap

### v0.1 — Indexer + enrichment + API + dashboard MVP *(in progress, target: end of June 2026)*

- [x] Rust workspace — `core` / `persistence` / `bootstrap` / `indexer` / `api` / `context` / `wasm`
- [x] Three-stage ingestion pipeline (`RpcListener` → `SignatureDispatcher` → `IndexerWorker`) with Prometheus instrumentation
- [x] DAMM v2 decoding — Anchor `event_cpi`, circle 1 events end-to-end (Swap, Liquidity, ClaimFee, ClaimReward)
- [x] Token enrichment daemon — metadata via Helius DAS, USD prices via Jupiter Price V3
- [x] HTTP API on axum — `/healthz`, `/api/pools`, `/api/tokens/{mint}`, embedded token data in pool responses
- [x] Four-role Postgres model — `yog_migrate` / `yog_indexer` / `yog_api` / `yog_context`, least-privilege enforced
- [x] One-shot migration binary (`yog-migrate`) and forward-only migration convention
- [x] Full Docker stack — 4 backend images + frontend image, profile-driven compose
- [x] CI on the Rust workspace — check / fmt / clippy / test / audit, plus a `sqlx --check` job against a real Postgres
- [ ] Next.js dashboard pages — overview and pools
- [ ] Scaleway deployment

### v0.2 — Signal Engine *(target: end of September 2026)*

Pattern detection on accumulated event data: TVL drain, fee yield spike, imbalance alerts, price impact creep. New `signals` crate, multi-channel alert delivery (webhook / email / Telegram). Server-Sent Events on the API for low-latency push to the dashboard.

### v0.3 — Auth and per-user pool watchlists *(target: end of November 2026)*

Multi-channel authentication (email, OAuth, Solana wallet), per-user pool watchlists, tier infrastructure with placeholder quotas. The `yog_api` Postgres role gains `INSERT/UPDATE` on user-facing tables. WASM activation reassessed at this point.

### v0.4 — Monetization *(target: February-March 2027)*

Stripe billing, public pricing tiers, API keys with rate limiting, enterprise / white-label offering.

### v0.5 — Extended Meteora coverage *(unscheduled)*

DLMM, DAMM v1, DAMM v1 Farm, Stake2Earn, LST, Multi-Token. Extends the existing extraction pipeline; no architectural changes expected.

### Post-v0.1 RPC upgrade

Move to Helius `transactionSubscribe` (Developer plan), Helius Startup Launchpad, or an equivalent gRPC provider (Shyft, Triton). Removes the watched-pool allowlist constraint and returns ingestion to fully protocol-centric coverage.

---

## Hosting

Production deployment targets **Scaleway** in the Paris region — a single instance running the four backend containers (`yog-migrate`, `yog-indexer`, `yog-api`, `yog-context`) plus the frontend and Caddy as reverse proxy, with a Managed PostgreSQL instance carrying the TimescaleDB extension and Object Storage for daily `pg_dump` backups. Approximate monthly cost: **~20 € HT**.

The full hosting layout, Docker Compose files, backup procedure, and provisioning checklist are documented in **[`Fiche_d_hébergement___Scaleway_full-stack.md`](./Fiche_d_hébergement___Scaleway_full-stack.md)**.

---

## Contributing

Contributions are welcome. Open an issue first to discuss what you'd like to change.

- For Rust conventions (formatting, clippy policy, sqlx offline workflow), see **[`crates/README.md`](./crates/README.md)**.
- For TypeScript conventions (lint, typecheck, test), see **[`web/README.md`](./web/README.md)**.

Before opening a PR, run the relevant local checks (mirrored by CI) and make sure your changes ship with their tests where applicable.

---

## License

[MIT](./LICENSE)