# crates/

This directory hosts the Rust workspace — the engine of yog-sothoth.

The workspace follows a **Domain-Driven Design** layout: domain types and contracts live in `core`, infrastructure and I/O live in dedicated adapter crates (`persistence` for Postgres, `bootstrap` for startup utilities). The four native binaries (`indexer`, `api`, `context`, `signals`) are thin assembly layers that wire the pieces together; a one-shot binary (`yog-migrate`) lives next to the migrations it applies.

**How the documentation is organised**: this README covers what is *inter-crate and common* — the dependency graph, the conventions, the database roles, the local workflows, and the cross-crate recipes (adding a protocol, adding an endpoint). Each substantial crate has its own README for its internals; each fact lives in exactly one place, so this file links rather than repeats. For the project-wide pitch and roadmap, see the [root README](../README.md).

---

## Conventions

The same principles guide every crate. They are not aspirational — the code is structured this way today, and a PR that breaks them is unlikely to be accepted.

- **Single responsibility per layer.** `core` knows no I/O. `persistence` knows no business logic. Binaries do no business logic and no SQL — they wire repositories into the runtime and route between them.
- **Repository traits in `core`, implementations in `persistence`.** The trait declares the contract; the implementation provides the SQL. Binaries depend on the trait, never on the concrete type. Where write side and read side have disjoint consumers, the trait is split per consumer (same `Pg*` struct behind both): the owning side keeps `*Repository`, the read lens is named by intent — `*Feed` (paginated time-ordered listing), `*Lookup` (point reads), `PoolCatalog` (see [`core/README.md`](./core/README.md#repository-traits)).
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
- **[`api` (`yog-api`)](./api/README.md)** — the read-only HTTP server. Sixteen endpoints, cursor pagination, RFC 9457 errors, and the shared SSE poller behind the live signal stream.
- **[`context` (`yog-context`)](./context/README.md)** — the enrichment daemon. Three workers: token metadata (Helius DAS), USD prices (Jupiter Price V3), and pool-account property backfill. The last one names no protocol — it iterates one `PoolAccountResolver` per protocol (cp-amm and DLMM today), each owning its queue and its satellite table.
- **[`signals` (`yog-signals`)](./signals/README.md)** — the signal engine. Batch detectors at per-detector cadence, stateless between ticks, cooldown-based dedup with severity escalation; three detectors today: swap-flow imbalance, spot-vs-oracle price deviation, TVL drain.
- **`wasm` (`yog-wasm`)** <a name="wasm-yog-wasm"></a> — WebAssembly target for the browser. **Currently a scaffold** — the default `cargo new --lib` template, not wired to `yog-core`. Making it functional requires a `wasm` feature on `yog-core`, conditional compilation on Solana-only modules, and abstracting `Pubkey` behind a neutral alias. Deferred; reassessed at v0.3 (auth).

---

## Database roles

All coordination between the binaries happens through the schema, and the schema enforces who may write what. Migrations are forward-only and flow exclusively through `yog-migrate`; each runtime process connects under its own least-privilege role:

| Role | Permissions | Used by |
|---|---|---|
| `yog_migrate` | DDL — owns the schema, applies migrations | `yog-migrate` binary, `cargo sqlx migrate run` |
| `yog_indexer` | `SELECT, INSERT, UPDATE` on event tables and on `pools` (table-level); `SELECT` on `watched_pools` | indexer |
| `yog_api` | `SELECT` across tables and VIEWs — nothing else | api |
| `yog_context` | `SELECT, INSERT, UPDATE` on `token_metadata` / `token_prices` and every per-protocol pool-properties satellite; `UPDATE` on the pool-property columns of `pools` — **the sole writer of account-derived properties**; `SELECT` on `pools` | context |
| `yog_signals` | `INSERT` (append-only) on `signals`; `SELECT` on its read VIEWs | signals |
| admin (e.g. `yog` superuser) | Full — provisioning, `cargo sqlx prepare`, ad-hoc operations | tooling only, never a running service |

The role split is the safety net, not a bug: calling a write method from the api process fails with `permission denied` from Postgres itself, by design. Provisioning mechanics (`setup_roles.sql`, default privileges) are documented in [`persistence/README.md`](./persistence/README.md#setup_rolessql).

**Where the grant stops and discipline starts.** `yog_context` is the sole writer of the account-derived properties *by grant*: on `pools` it holds `UPDATE` on four named columns only (`token_a_mint`, `token_b_mint`, `fee_bps`, `needs_refresh`), which is what keeps `protocol` and the `*_seen_at` timestamps the indexer's. The reverse is not enforced: `yog_indexer` holds table-level `UPDATE` on `pools`, so nothing in Postgres stops it writing a property value — that it only ever writes identity and raises `needs_refresh` is a property of the code, not of the schema. The satellites are the enforced half: the indexer has no grant on them at all. Both directions are pinned by `tests/privileges.rs`.

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

# 2. Provision everything: roles + structural privileges, then the migrations,
#    then the watched-pools allowlist. Idempotent — safe to re-run, and safe
#    to run against a second database of the same cluster.
#    (Reads DATABASE_URL_ADMIN and DATABASE_URL_MIGRATE from .env.)
cargo run -p yog-persistence --bin yog-migrate -- bootstrap

#    …or one step at a time:
#      … -- setup-roles          (DATABASE_URL_ADMIN)
#      … -- migrate              (DATABASE_URL_MIGRATE, the default with no arg)
#      … -- seed-watched-pools   (DATABASE_URL_ADMIN)

# 3. Review the allowlist before starting the indexer. The seed ships ONE pool
#    — SOL-USDC, the routing hub, the only pick whose rationale does not decay.
#    A pool that has gone quiet subscribes fine and collects nothing, so add
#    pools trading NOW: see persistence/README.md → "Choosing pools to watch".
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" -c \
    "SELECT pool_address, note FROM watched_pools WHERE active;"

# 4. Run the binary you're working on
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
cargo test -p yog-persistence --features integration-tests
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
- Create a top-level struct (e.g. `MeteoraDlmm`) and implement `EventExtractor`. Its input is a `OnChainTransaction` — the neutral shape every source adapter fills — so no protocol handler ever names a transport.
- Add a new branch to `ExtractionDispatcher::extract` that routes the new `Protocol` variant to the new struct.

**Domain side**:

- Create per-event modules under `domain/<platform>/<product>/{event_kind}/` with `model.rs` and `repository.rs`. Event structs are prefixed with the protocol (e.g. `MeteoraDlmmSwapEvent`), as are their cursor types, which take the event's full name (`MeteoraDlmmSwapEventCursor`).
- Add the sub-enum `<Platform><Product>Event` in `domain/<platform>/<product>.rs` with one variant per event kind.
- Add an outer variant in `DomainEvent` (`domain/domain_event.rs`) and update the accessor methods (`pool_address`, `signature`, `timestamp`, `protocol`, `kind`) to match.

**Account side** — the properties events never carry (mints, base fee, fee shape):

- Decode the pool account in `application/decoder/<platform>/<product>.rs`, returning a `DecodedPoolAccount`. Guard on the Anchor discriminator as well as the program id — neither is redundant (see [`core/README.md`](./core/README.md#responsibilities)).
- Add a branch to `decode_pool_account` (`application/decoder.rs`) routing the new `Protocol`.
- Add the matching variants to the two-level `PoolAccountProperties` (write side, `domain/pool_account/`) and `PoolProperties` (read side, `domain/pool_properties/`). Both are matched exhaustively downstream, so the compiler points at every site that must follow.
- Ground the decoder on **real mainnet accounts** before trusting it: a synthetic test builds the buffer with the same constants the decoder reads it with, so both agree on a wrong offset. Fixtures go under `core/tests/fixtures/<protocol>/accounts/`.

### 2. In `persistence`

- Add a migration that creates the per-protocol tables (`<platform>_<product>_<event_kind>_events`). Each table holds only the columns relevant to the protocol. Include `GRANT INSERT, UPDATE ON <new_table> TO yog_indexer;`.
- Add the **pool-properties satellite** `<platform>_<product>_pool_properties` in the same or a following migration: one row per pool, primary-keyed on `pool_address`, holding what exists for this protocol only — `pools` stays the cross-protocol registry. Copy the shape from migration `039`/`040`: a `protocol TEXT NOT NULL GENERATED ALWAYS AS ('<the protocol>') STORED` column plus a composite `FOREIGN KEY (pool_address, protocol) REFERENCES pools (pool_address, protocol) ON DELETE CASCADE` **instead of** the single-column FK, so the row cannot claim a protocol the registry disagrees with. Grant it to `yog_context` (its sole writer), not to `yog_indexer`.
- Add every new table's line to the privilege matrix in `tests/privileges.rs`. The suite asserts in both directions, so a missing GRANT and an extra one both fail — read the failure as a question before editing either side.
- Extend the cross-protocol VIEWs with a new `UNION ALL` branch per VIEW (in a new migration redefining them), the `protocol` literal injected.
- Implement the new `Pg<Platform><Product><EventKind>EventRepository` traits in `persistence/src/repositories/<platform>/<product>/`. Follow the `Row + TryFrom<XxxRow> for XxxDomain` convention.
- Implement the satellite's `PoolAccountResolver` (write + queue) and `PoolPropertiesLookup` (read) on the same `Pg*` struct. ⚠️ Its `list_unresolved` **must** filter on its own protocol: "has no satellite row yet" is one of the candidate conditions, and that is permanently true of every pool of every *other* protocol — get it wrong and the queue starves behind pools this resolver can never store.
- Regenerate `.sqlx/` (`cd crates/persistence && cargo sqlx prepare -- --all-targets --all-features`).
- Re-export the new repositories from `lib.rs`.

### 3. In `indexer`

- Create a sub-persistor `<Platform><Product>EventPersistor` under `application/services/<platform>/<product>/event_persistor.rs`. It owns the per-protocol repos plus an `Arc<PoolMaintenance>`. Its `persist` method matches on the protocol's sub-enum and dispatches to per-variant `persist_<kind>` methods.
- Add a new branch in `EventPersistor::persist` that delegates `DomainEvent::<NewProtocol>(e)` to the new sub-persistor.
- In `bootstrap/daemon.rs::init_event_persistor`, instantiate the new sub-persistor with its repos plus the shared `PoolMaintenance`, and wire it into the top-level `EventPersistor`.

### 4. In `context`

- Push the new `Pg*` resolver into the `pool_account_resolvers` vec in `bootstrap/daemon.rs`. That is the whole wiring: `PoolAccountWorker` names no protocol and iterates the vec — DLMM was added in exactly one line here and the worker was untouched.

### 5. In `api` (when read access is needed)

- If the protocol introduces new event kinds the API wants to expose, add a service under `application/services/`. A service goes under `services/<platform>/<product>/` only when its repository, params and result are irreducibly that product's; cross-protocol services stay at the root and must not name a protocol in their constructor.
- Add handlers and DTOs as needed. For a cross-protocol read surface, point the handler at the matching VIEW; for protocol-specific detail, point at the table directly.
- Add the protocol's block to `http/dto/response/pool.rs` — an optional field named after the protocol, alongside its siblings. The `PoolProperties` destructuring there is irrefutable by construction, so this one is compiler-forced rather than optional.

### 6. Tests

Add fixture transactions under `core/tests/fixtures/` — they stay in `core`, they are data — and extend the extraction tests in `indexer/src/infra/rpc/fixture_pipeline_tests.rs`, which live beside the adapter that turns a fixture into an `OnChainTransaction`. Add the account fixtures mentioned in step 1, and the privilege-matrix lines mentioned in step 2.

### What stays narrow

There is no central registry. A protocol is added by writing isolated per-protocol code plus a fixed, short list of dispatch points — one per concern, each a single branch or a single line:

| Dispatch point | Crate | Concern |
|---|---|---|
| `ExtractionDispatcher::extract` | `core` | transaction → events |
| `decode_pool_account` | `core` | account bytes → properties |
| `EventPersistor::persist` | `indexer` | event → sub-persistor |
| `init_event_persistor` (`bootstrap/daemon.rs`) | `indexer` | sub-persistor instantiation |
| `pool_account_resolvers` vec (`bootstrap/daemon.rs`) | `context` | property backfill |
| `PoolProperties` match in `http/dto/response/pool.rs` | `api` | detail wire shape |

The two-level enums (`DomainEvent`, `PoolAccountProperties`, `PoolProperties`) are matched exhaustively, so adding a variant breaks the build at each site that must follow instead of silently dropping the protocol.

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
