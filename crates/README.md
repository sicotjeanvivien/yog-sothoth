# crates/

This directory hosts the Rust workspace — the engine of yog-sothoth.

The workspace follows a **Domain-Driven Design** layout: domain types and contracts live in `core`, infrastructure and I/O live in dedicated adapter crates (`persistence` for Postgres, `bootstrap` for startup utilities). The four native binaries (`indexer`, `api`, `context`, `signals`) are thin assembly layers that wire the pieces together; a one-shot binary (`yog-migrate`) lives next to the migrations it applies.

**How the documentation is organised**: this README covers what is *inter-crate and common* — the dependency graph, the conventions, the database roles, the local workflows, and the cross-crate recipes (adding a protocol, adding an endpoint). Each substantial crate has its own README for its internals; each fact lives in exactly one place, so this file links rather than repeats. For the project-wide pitch and roadmap, see the [root README](../README.md).

---

## Conventions

The same principles guide every crate. They are not aspirational — the code is structured this way today, and a PR that breaks them is unlikely to be accepted.

- **Single responsibility per layer.** `core` knows no I/O. `persistence` knows no business logic. Binaries do no business logic and no SQL — they wire repositories into the runtime and route between them.
- **Repository traits in `core`, implementations in `persistence`.** The trait declares the contract; the implementation provides the SQL. Binaries depend on the trait, never on the concrete type. Where write side and read side have disjoint consumers, the trait is split per consumer (same `Pg*` struct behind both).
- **Typed errors at every layer boundary.** `RepositoryError` at the persistence boundary, `ApiError` at the HTTP boundary, typed pipeline errors at each indexer stage. A `?` operator that crosses a boundary maps the error explicitly.
- **Skip-and-log over abort-and-die.** Partial failures (a malformed event, a failed insert, a failed detector tick) are logged, counted, and stepped over. Loop-level failures (closed channel, exhausted semaphore, panic) bubble up and trigger a clean shutdown via a shared `CancellationToken`.
- **Domain types are infra-neutral.** Addresses are `Pubkey`. Decimal prices are `rust_decimal::Decimal`. Lossless `u128` values are `BigDecimal` only at the persistence boundary (`NUMERIC(39, 0)` in Postgres). No `sqlx::types` leaks into `core`.
- **Per-protocol typing all the way down.** Domain events, SQL tables, repositories and sub-persistors are all scoped per `(platform, protocol)` pair — `MeteoraDammV2SwapEvent`, `meteora_damm_v2_swap_events`, `PgMeteoraDammV2SwapEventRepository`. The `DomainEvent` enum is two-level: outer variant per protocol, inner sub-enum per event kind. New protocols add a new outer variant without polluting the existing ones. Cross-protocol concepts (`Pool`, `TokenPrice`, `Signal`, …) stay generic, single-table, with a discriminating column where useful.

---

## Structure

```
crates/
├── core/          ← shared library: domain types, AMM math, protocol extraction
├── persistence/   ← Postgres adapter: repository impls, migrations, yog-migrate
├── bootstrap/     ← shared startup utilities: env helpers, SecretUrl, init_rustls/tracing
├── indexer/       ← binary: Solana RPC ingestion → DB
├── api/           ← binary: axum HTTP server + SSE over the indexed data
├── context/       ← binary: token/pool enrichment (Helius DAS, Jupiter, cp-amm accounts)
├── signals/       ← binary: batch detector engine emitting typed signals
└── wasm/          ← WASM build target (scaffold — deferred)
```

The dependency graph is strict and one-directional:

```
                       ┌──────────┐
                       │   core   │  no I/O, wasm-compatible
                       └────▲─────┘
                            │
              ┌─────────────┼─────────────┬─────────┐
              │             │             │         │
        ┌─────┴─────┐ ┌─────┴─────┐  ┌────┴────┐    │
        │persistence│ │ bootstrap │  │  wasm   │    │
        └─────▲─────┘ └─────▲─────┘  └─────────┘    │
              │             │                       │
              └──────┬──────┘                       │
                     │                              │
      ┌──────────┬───┴──────┬───────────┐           │
      │          │          │           │           │
 ┌────┴────┐ ┌───┴───┐ ┌────┴────┐ ┌────┴────┐      │
 │ indexer │ │  api  │ │ context │ │ signals │      │
 └─────────┘ └───────┘ └─────────┘ └─────────┘      │
                                                    │
                                          (no binary depends on wasm)
```

`core` knows nothing about Postgres, axum, or HTTP clients. It declares traits; the adapters and binaries implement and consume them. Each binary depends only on `core` (types + traits), `persistence` (concrete repos), and `bootstrap` (startup helpers).

---

## The crates

- **[`core` (`yog-core`)](./core/README.md)** — pure logic and domain types. Domain entities and every repository trait, the two-level `DomainEvent`, the Anchor `event_cpi` extraction pipeline, the `SignalDetector` contract, AMM math, pagination primitives. No I/O.
- **[`persistence` (`yog-persistence`)](./persistence/README.md)** — the Postgres adapter. `Pg*` repository implementations, the forward-only migration suite, the `yog-migrate` binary, the SQLx offline cache, the query-shape policy (inline `query!` / VIEW / `QueryBuilder`), and the `watched_pools` operational reference.
- **`bootstrap` (`yog-bootstrap`)** — shared startup utilities, deliberately tiny: env parsing primitives, the redacting `SecretUrl`, `ConfigError`, `init_rustls()`, `init_tracing()`. The decision rule for adding anything: *does this run identically in every binary's `main()`?* If it varies even slightly, it stays in the binary. (Small enough that this paragraph is its documentation.)
- **[`indexer` (`yog-indexer`)](./indexer/README.md)** — the ingest daemon. Three-stage pipeline (WebSocket listener → signature dispatcher → bounded worker), `TransactionProcessor`, per-protocol sub-persistors, Prometheus metrics.
- **[`api` (`yog-api`)](./api/README.md)** — the read-only HTTP server. Fourteen endpoints, cursor pagination, RFC 9457 errors, and the shared SSE poller behind the live signal stream.
- **[`context` (`yog-context`)](./context/README.md)** — the enrichment daemon. Three workers: token metadata (Helius DAS), USD prices (Jupiter Price V3), and pool-account property backfill (mints, fee config from cp-amm accounts).
- **[`signals` (`yog-signals`)](./signals/README.md)** — the signal engine. Batch detectors at per-detector cadence, stateless between ticks, cooldown-based dedup with severity escalation; first two detectors: swap-flow imbalance and spot-vs-oracle price deviation.
- **`wasm` (`yog-wasm`)** <a name="wasm-yog-wasm"></a> — WebAssembly target for the browser. **Currently a scaffold** — the default `cargo new --lib` template, not wired to `yog-core`. Making it functional requires a `wasm` feature on `yog-core`, conditional compilation on Solana-only modules, and abstracting `Pubkey` behind a neutral alias. Deferred; reassessed at v0.2.

---

## Database roles

All coordination between the binaries happens through the schema, and the schema enforces who may write what. Migrations are forward-only and flow exclusively through `yog-migrate`; each runtime process connects under its own least-privilege role:

| Role | Permissions | Used by |
|---|---|---|
| `yog_migrate` | DDL — owns the schema, applies migrations | `yog-migrate` binary, `cargo sqlx migrate run` |
| `yog_indexer` | `SELECT, INSERT, UPDATE` on event tables and pool registry; `SELECT` on `watched_pools` | indexer |
| `yog_api` | `SELECT` across tables and VIEWs — nothing else | api |
| `yog_context` | `SELECT, INSERT, UPDATE` on `token_metadata` / `token_prices`; `UPDATE` on pool-property columns; `SELECT` on `pools` | context |
| `yog_signals` | `INSERT` (append-only) on `signals`; `SELECT` on its read VIEWs | signals |
| admin (e.g. `yog` superuser) | Full — provisioning, `cargo sqlx prepare`, ad-hoc operations | tooling only, never a running service |

The role split is the safety net, not a bug: calling a write method from the api process fails with `permission denied` from Postgres itself, by design. Provisioning mechanics (`setup_roles.sql`, default privileges) are documented in [`persistence/README.md`](./persistence/README.md#setup_rolessql).

---

## Local development

Two workflows are supported.

### A. Docker (default, easiest)

```bash
# Postgres only — when running native cargo run alongside
docker compose up -d

# Full backend stack (postgres + migrate + indexer + api + context + signals)
docker compose --profile backend up -d --build

# Everything including the Next.js dashboard
docker compose --profile full up -d --build

# Tail a service's logs
docker compose logs -f yog-indexer

# Tear down with volume removal (full reset)
docker compose --profile full down -v
```

### B. Native `cargo run` (faster inner loop)

```bash
# 1. Start Postgres only
docker compose up -d

# 2. Provision the roles (one-time, as superuser)
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f crates/persistence/setup_roles.sql

# 3. Apply migrations (as yog_migrate)
cargo run -p yog-persistence --bin yog-migrate

# 4. Seed the watched-pools allowlist (one-time)
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f scripts/seed_watched_pools.sql

# 5. Run the binary you're working on
cargo run -p yog-indexer    # or yog-api, yog-context, yog-signals

# Hit the api
curl http://127.0.0.1:5000/healthz
curl http://127.0.0.1:5000/api/pools | jq
```

**Port gotcha:** the `.env` URLs use `localhost:5433` (the host-side Docker
port mapping). Inside the Docker network the port is `5432` — each compose
service rewrites the URL in its `command`. Natively you talk to `localhost:5433`.

### Building, testing, linting

```bash
cargo build
cargo fmt --all
cargo test --workspace --all-features

# Native crates only — yog-wasm is excluded (deferred scaffold)
cargo clippy -p yog-api -p yog-core -p yog-context -p yog-indexer \
    -p yog-persistence -p yog-signals \
    --all-targets --all-features -- -D warnings

# DB-backed integration tests (need live Postgres, see persistence/README.md)
cargo test -p yog-persistence --features integration-tests -- --include-ignored
```

The Rust version is pinned in `rust-toolchain.toml` at the repo root — don't override it.

---

## CI

GitHub Actions runs on every push and PR to `main`:

- **`crates.yml`** — Rust workspace: `check`, `fmt`, `clippy -D warnings`, `test`, `audit`, `sqlx-check` (spins up TimescaleDB, applies migrations, verifies the committed `.sqlx/` cache)
- **`web-quality.yml`** / **`web-docker.yml`** — the frontend (see [`web/README.md`](../web/README.md))

---

## Adding a new protocol

The "voie 3" per-protocol shape means a new protocol creates new domain types, new SQL tables, new repositories, and a new sub-persistor — but each step follows a clean pattern, and the dispatch points stay narrow.

### 1. In `core`

**Extraction side**:

- Create a module under `application/extraction/<platform>/<product>/` (e.g. `extraction/meteora/dlmm/`). Split responsibilities following the DAMM v2 pattern: `events.rs` for wire events (borsh mirrors), `extractor.rs` for walking inner instructions, `translator.rs` for wire → domain translation.
- Create a top-level struct (e.g. `MeteoraDlmm`) and implement `EventExtractor`.
- Add a new branch to `ExtractionDispatcher::extract` that routes the new `Protocol` variant to the new struct.

**Domain side**:

- Create per-event modules under `domain/<platform>/<product>/{event_kind}/` with `model.rs` and `repository.rs`. Event structs are prefixed with the protocol (e.g. `MeteoraDlmmSwapEvent`), as are their cursor types (`MeteoraDlmmSwapCursor`).
- Add the sub-enum `<Platform><Product>Event` in `domain/<platform>/<product>.rs` with one variant per event kind.
- Add an outer variant in `DomainEvent` (`domain/domain_event.rs`) and update the accessor methods (`pool_address`, `signature`, `timestamp`, `protocol`, `kind`) to match.

### 2. In `persistence`

- Add a migration that creates the per-protocol tables (`<platform>_<product>_<event_kind>_events`). Each table holds only the columns relevant to the protocol. Include `GRANT INSERT, UPDATE ON <new_table> TO yog_indexer;`.
- Extend the cross-protocol VIEWs with a new `UNION ALL` branch per VIEW (in a new migration redefining them), the `protocol` literal injected.
- Implement the new `Pg<Platform><Product><EventKind>EventRepository` traits in `persistence/src/repositories/<platform>/<product>/`. Follow the `Row + TryFrom<XxxRow> for XxxDomain` convention.
- Regenerate `.sqlx/` (`cd crates/persistence && cargo sqlx prepare`).
- Re-export the new repositories from `lib.rs`.

### 3. In `indexer`

- Create a sub-persistor `<Platform><Product>EventPersistor` under `application/services/<platform>/<product>/event_persistor.rs`. It owns the per-protocol repos plus an `Arc<PoolMaintenance>`. Its `persist` method matches on the protocol's sub-enum and dispatches to per-variant `persist_<kind>` methods.
- Add a new branch in `EventPersistor::persist` that delegates `DomainEvent::<NewProtocol>(e)` to the new sub-persistor.
- In `bootstrap/daemon.rs::init_event_persistor`, instantiate the new sub-persistor with its repos plus the shared `PoolMaintenance`, and wire it into the top-level `EventPersistor`.

### 4. In `api` (when read access is needed)

- If the protocol introduces new event kinds the API wants to expose, add a service under `application/services/`.
- Add handlers and DTOs as needed. For a cross-protocol read surface, point the handler at the matching VIEW; for protocol-specific detail, point at the table directly.

### 5. Tests

Add fixture transactions under `core/tests/fixtures/` (one per recognized signature for the new protocol) and extend the extraction integration tests in `core/tests/`.

### What stays narrow

Each crate has exactly one dispatch point per protocol:

- `ExtractionDispatcher::extract` (`core`) — one branch
- `EventPersistor::persist` (`indexer`) — one branch
- `init_event_persistor` (`indexer`, `bootstrap/daemon.rs`) — one instantiation block

Everything else is per-protocol-isolated code. No central registry to update beyond these three points.

---

## Adding a new API endpoint

For endpoints that read existing data (no new tables, no new domain types), the workflow is contained:

### 1. Extend the relevant repository trait in `core`

If the endpoint needs a query that doesn't exist yet, add the method to the trait in `core/src/domain/<aggregate>/repository.rs`. Document the ordering and pagination contract.

### 2. Implement the new method in `persistence`

Add the SQL in the corresponding `Pg*Repository` impl (see the [query-shape policy](./persistence/README.md#choosing-how-to-write-a-query) — inline `query!`, VIEW, or `QueryBuilder`). Regenerate `.sqlx/`.

### 3. Add the handler in `api`

- Create or extend a module under `api/src/http/handlers/`.
- Create request/response DTOs in `api/src/http/dto/` (request validation happens here, before any DB call).
- Mount the route in `http.rs::build_router`.
- Reuse `ApiError` for error mapping; the `From<RepositoryError>` impl handles repository failures uniformly.

### 4. Verify

```bash
cargo run -p yog-api
curl http://127.0.0.1:5000/api/<your-endpoint> | jq
```

### Conventions

- **Pagination** — all collection endpoints use cursor-based pagination via `Page<T>` and a domain-specific cursor type. Default `limit = 50`, hard cap `200`.
- **Error responses** — RFC 9457 Problem Details (see [`api/README.md`](./api/README.md#error-responses)).
- **Validation** — client-supplied data is validated at the handler boundary, before any DB call.
- **Pubkeys** — base58 strings in responses (matching `Pubkey::Display`); same format on input.
- **Timestamps** — RFC3339 / ISO8601.
