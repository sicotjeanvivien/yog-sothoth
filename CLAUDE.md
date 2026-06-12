# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Protocol-centric observer of Meteora's on-chain activity on Solana. It subscribes to Meteora program IDs over WebSocket, decodes Anchor `event_cpi` emissions, reconstructs AMM state, and persists it to TimescaleDB. An HTTP API and a Next.js dashboard read that data. Pools are *discovered* from the transaction stream, not configured upfront — the `pools` table records what was *seen*, not a watchlist.

Four backend processes share one Postgres database and never call each other — all coordination is through the schema: `indexer` (ingest), `context` (token/price enrichment), `api` (axum HTTP), `web` (Next.js BFF).

## Where the real documentation lives

This file is a map, not the territory. Two in-repo READMEs are the authoritative, maintained architecture docs — read them before non-trivial work:

- **`crates/README.md`** — the Rust workspace bible: per-crate responsibilities, the extraction pipeline, the per-protocol table strategy, DB roles, and step-by-step recipes for *Adding a new protocol* and *Adding a new API endpoint*. Follow those recipes verbatim; they list every dispatch point that must change.
- **`web/README.md`** — the Next.js frontend (BFF architecture, feature flags, i18n).

## Commands

All Rust commands run from the repo root. Toolchain is pinned in `rust-toolchain.toml` (don't override it).

```bash
# Build / format
cargo build
cargo fmt --all

# Lint — native crates only (yog-wasm is excluded; it's a deferred scaffold)
cargo clippy -p yog-api -p yog-core -p yog-context -p yog-indexer -p yog-persistence \
    --all-targets --all-features -- -D warnings

# Test — workspace unit tests (DB-free)
cargo test --workspace --all-features
cargo test -p yog-core extraction          # a single crate / filter
cargo test -p yog-core -- --exact <test>   # one exact test

# Integration tests are DB-backed and #[ignore]d by default — they need a live Postgres:
cargo test -p yog-persistence --features integration-tests -- --include-ignored

# Run a binary natively (see "Local dev" for the DB it expects)
cargo run -p yog-indexer        # or yog-api, yog-context
cargo run -p yog-persistence --bin yog-migrate   # apply migrations (as yog_migrate)
```

Web (run from `web/`): `npm run dev`, `npm run build`, `npm run lint`, `npm run typecheck`, `npm test` (vitest).

## SQLx offline cache — easy to forget, breaks CI

Queries use `sqlx::query!`/`query_as!` macros validated at compile time against committed snapshots in `crates/persistence/.sqlx/`. The workspace builds with `SQLX_OFFLINE=true`. **After adding or changing any `sqlx::query!` call you must regenerate the cache and commit it**, or the `sqlx-check` CI job fails:

```bash
cd crates/persistence && cargo sqlx prepare
```

## Architecture rules that are enforced, not aspirational

A PR that breaks these is unlikely to be accepted (full list in `crates/README.md` → *Conventions*):

- **Strict one-directional layering.** `core` has no I/O (no Postgres, no axum, no HTTP, wasm-compatible). `persistence` has no business logic. Binaries do no business logic and no SQL — they wire repositories into the runtime. Dependency graph: binaries → `core` + `persistence` + `bootstrap`; everything → `core`.
- **Repository traits in `core`, `Pg*` impls in `persistence`.** Binaries depend on the trait, never the concrete type.
- **Typed errors at every boundary** (`RepositoryError`, `ApiError`, per-stage indexer errors). A `?` crossing a boundary maps the error explicitly.
- **Skip-and-log over abort-and-die.** Per-event / per-signature failures are logged + counted (Prometheus) and stepped over. Only loop-level failures (closed channel, exhausted semaphore, panic) bubble up and trigger graceful shutdown via the shared `CancellationToken`.
- **Domain types are infra-neutral.** Addresses are `Pubkey`, decimal prices are `rust_decimal::Decimal`. Lossless `u128` becomes `BigDecimal` (`NUMERIC(39,0)`) *only* at the persistence boundary. No `sqlx::types` in `core`.

## Per-protocol "voie 3" — the dominant design decision

Everything is typed per `(platform, protocol, event_kind)`, all the way down: domain events (`MeteoraDammV2SwapEvent`), SQL tables (`meteora_damm_v2_swap_events`), repositories (`PgMeteoraDammV2SwapEventRepository`), and indexer sub-persistors. `DomainEvent` is a **two-level enum**: outer variant per protocol, inner sub-enum per event kind — `DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(..))`.

- Each table holds only columns relevant to its protocol — no NULL columns for incompatible fields, no JSONB blob.
- Cross-protocol reads ("all swaps for a pool") go through SQL **VIEW**s (`swap_events`, `liquidity_events`, …) defined at the bottom of the baseline migration, each a `UNION ALL` with a synthesised `protocol` column. Protocol-specific columns are *not* in the VIEWs — read the underlying table when you need them.
- Adding a protocol touches exactly **three dispatch points** (`ExtractionDispatcher::extract`, `EventPersistor::persist`, the persistor wiring in `Daemon::new`); everything else is isolated per-protocol code. Cross-protocol concepts (`Pool`, `PoolCurrentState`, `TokenMetadata`, `TokenPrice`) stay generic, single-table.

## Database privilege model

Migrations are **forward-only** (committed migrations never change; no `.down.sql`; rollback = restore from backup). All DDL flows through one binary (`yog-migrate`) under the `yog_migrate` role. Each runtime process connects under its own least-privilege role, enforced by Postgres itself:

| Role | Rights | Process |
|---|---|---|
| `yog_migrate` | DDL, owns schema | `yog-migrate` |
| `yog_indexer` | RW on event/pool tables, RO `watched_pools` | indexer |
| `yog_api` | RO everywhere | api |
| `yog_context` | RW on `token_metadata` / `token_prices`, RO `pools` | context |

Consequence: calling `insert` from the `api` process fails with `permission denied` *by design* — the role split is the safety net, not a bug. When you add a table in a migration, add its `GRANT INSERT, UPDATE ... TO yog_indexer;` in the same migration (`SELECT` is covered by default privileges in `setup_roles.sql`).

## Local development

Two workflows (details in `crates/README.md` → *Local development*):

```bash
# A. Docker (easiest) — Postgres only, or the full stack via compose profiles
docker compose up -d                                   # Postgres only
docker compose --profile backend up -d --build         # + migrate/indexer/api/context
docker compose --profile full up -d --build            # + web dashboard

# B. Native cargo (faster inner loop), with Postgres in Docker:
docker compose up -d
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" -f crates/persistence/setup_roles.sql
cargo run -p yog-persistence --bin yog-migrate
cargo run -p yog-indexer
```

**Port gotcha:** the `.env` URLs use `localhost:5433` (the host-side Docker port mapping `5433:5432`). Inside the Docker network the port is `5432` — each compose service rewrites `@localhost:5433/` to `@postgres:5432/` in its `command`. When running natively you talk to `localhost:5433`.

## Observability

The indexer exposes Prometheus metrics on `:9000/metrics`. The API uses RFC 9457 Problem Details (`application/problem+json`) for errors and correlates 500s via the `x-request-id` header. Collection endpoints use opaque base64 cursor pagination (`Page<T>`, default limit 50, hard cap 200).
