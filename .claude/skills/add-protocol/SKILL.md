---
name: add-protocol
description: Add a new Meteora/AMM protocol (or product) to Yog-Sothoth following the "voie 3" per-protocol pattern. Use when the user wants to support a new on-chain protocol/product end-to-end — new domain events, SQL tables, repositories, indexer sub-persistor, and optional API surface. A map of the recipe in crates/README.md — file paths, order, commands and the checklist; every rule stays in the recipe.
---

# Add a new protocol (voie 3)

The rules, and why each one holds, live in one place: `crates/README.md` → *Adding a new
protocol*. Read the section a step names **before** doing that step. This skill carries only
what the recipe does not: exact paths, the order, the commands, the checklist. It restates
no rule — if a step below seems to need one, the rule belongs in the recipe, not here.

Template for every layer: the `meteora/damm_v2` implementation.

## Before you start

Ask the user, don't assume: the platform, the product and the snake_case `Protocol` string
(e.g. `meteora`, `dlmm`, `meteora_dlmm`); the program id (base58); the event kinds to
support.

Then read where the protocol stands today — the code says it, this file does not:

```bash
sed -n '/pub enum Protocol/,/^}/p' crates/core/src/domain/protocol/model.rs   # named by the enum?
grep -rn -A2 'fn is_implemented' crates/core/src/application/extraction/   # false = stub
sed -n '/pub enum DomainEvent/,/^}/p' crates/core/src/domain/domain_event.rs   # has events?
grep -rli '<product>' crates/*/src web/src | sort   # every file that already names it, layer by layer
```

## The dispatch points

The list is the table *What stays narrow* in `crates/README.md`, with the concern of each.
Where each one lives, in the table's order — if the table has a row this one lacks, the
table is right: add the row here.

| Dispatch point | File |
|---|---|
| `ExtractionDispatcher::extract` | `crates/core/src/application/extraction/extraction_dispatcher.rs` |
| `ExtractionDispatcher::implemented_protocols` | same file |
| `decode_pool_account` | `crates/core/src/application/decoder.rs` |
| `PgAnalyticRepository::write_cycle` | `crates/persistence/src/repositories/analytic.rs` |
| `EventPersistor::persist` | `crates/indexer/src/application/services/event_persistor.rs` |
| `init_event_persistor` | `crates/indexer/src/bootstrap/daemon/init.rs` |
| `pool_account_resolvers` vec | `crates/context/src/bootstrap/daemon.rs` |
| `From<domain::Protocol>` for `ProtocolResponse` | `crates/api/src/http/dto/response/protocol.rs` |
| `spot_price_a_in_b` | `crates/signals/src/application/detectors/price_oracle_deviation.rs` |
| `formatProtocolLabel` | `web/src/lib/format/format-protocol.ts` |
| `PROTOCOL_OF` | `web/src/components/dashboard/overview/protocol-activity.tsx` |

## Step 1 — `core`

Rules: *1. In `core`*.

- A platform new to the tree first gets its module in
  `crates/core/src/application/extraction.rs`, `…/application/decoder.rs`,
  `crates/core/src/domain.rs`, `crates/persistence/src/repositories.rs` and
  `crates/indexer/src/application/services.rs`, each with its `<platform>.rs` — declared
  and re-exported the way `meteora` is there (rules: *Conventions*, module visibility).
- Extraction:
  - `crates/core/src/application/extraction/<platform>/<product>/` — `events.rs`,
    `extractor.rs`, `translator.rs`; template `extraction/meteora/damm_v2/`.
  - `crates/core/src/application/extraction/<platform>/<product>.rs` — the struct
    implementing `EventExtractor`; template `extraction/meteora/damm_v2.rs`, declared in
    `extraction/<platform>.rs`.
  - `extraction_dispatcher.rs` — the first two rows of the table.
- Domain:
  - `crates/core/src/domain/<platform>/<product>/<event_kind>/` — `model.rs`,
    `repository.rs`; the sub-enum in `domain/<platform>/<product>.rs`; template
    `domain/meteora/damm_v2/`.
  - `crates/core/src/domain/domain_event.rs` — the outer variant and its accessors.
  - `crates/core/src/domain/protocol/model.rs` — only for a protocol the enum does not name
    yet.
- Account side:
  - `crates/core/src/application/decoder/<platform>/<product>.rs`; template
    `decoder/meteora/`.
  - `crates/core/src/application/decoder.rs` — its row of the table.
  - `crates/core/src/domain/pool_account/` — the `PoolAccountProperties` variant.
  - `crates/core/tests/fixtures/<product>/accounts/` — real mainnet accounts.

## Step 2 — `persistence`

Rules: *2. In `persistence`*; the migration itself with `/add-migration`.

- `crates/persistence/migrations/` — the migrations step 2 of the recipe asks for.
- `crates/persistence/tests/privileges.rs` — the privilege matrix.
- `crates/persistence/src/repositories/<platform>/<product>/` — the event repositories;
  template `repositories/meteora/damm_v2/`.
- The satellite's resolver — template `repositories/meteora/dlmm/pool_properties.rs`
  (`PgMeteoraDlmmPoolPropertiesRepository`, implementing `PoolAccountResolver`).
- `crates/persistence/src/repositories/analytic/<protocol>.rs` and its `.sql` — template
  `analytic/meteora_damm_v2.rs` (`upsert_births`) and `meteora_damm_v2_tranche.sql`; its
  row of the table.
- `crates/persistence/src/lib.rs` — the re-exports.
- `crates/persistence/.sqlx/` — regenerated, see *Commands*.

## Step 3 — `indexer`

Rules: *3. In `indexer`*.

- `crates/indexer/src/application/services/<platform>/<product>/event_persistor.rs` —
  template `services/meteora/damm_v2/event_persistor.rs` (`MeteoraDammV2EventPersistor`,
  `DammV2Repos`, `PoolMaintenance`).
- `services/event_persistor.rs` and `bootstrap/daemon/init.rs` — their rows of the table.

## Step 4 — `context`

Rules: *4. In `context`*.

- `crates/context/src/bootstrap/daemon.rs` — its row of the table.

## Step 5 — `api`, only if read access is needed

Rules: *5. In `api` (when read access is needed)*; each route: *Adding a new API endpoint*.

- `crates/api/src/application/services/` — a service, if any.
- `crates/api/src/http/dto/response/protocol.rs` — its row of the table, then the contract
  (*Commands*).

## Step 6 — `signals` and `web`

Rules: *6. In `signals` and `web`*.

- `crates/signals/src/application/detectors/price_oracle_deviation.rs` — its row of the table.
- `crates/signals/src/bootstrap/daemon.rs` — the detectors' instances, if any.
- `web/src/lib/format/format-protocol.ts` — its row of the table.
- The menu, if the product shows there: `web/src/components/dashboard/sidebar/sidebar-keys.ts`,
  `sidebar-nav.ts`, `web/messages/*/dashboard.json`, and the `PROTOCOL_OF` row.

## Step 7 — Tests

Rules: *7. Tests*.

- `crates/core/tests/fixtures/<product>/` — the transactions.
- `crates/indexer/src/infra/rpc/tests/fixture_pipeline_tests.rs` — the extraction tests.
- `crates/indexer/src/infra/rpc/tests/extraction_oracle_tests.rs` — the oracle.

## Commands

Format and lint: `crates/README.md` → *Building, testing, linting* (its clippy line names
every crate). Tests:

```bash
(cd crates/persistence && cargo sqlx prepare -- --all-targets --all-features)  # first: builds are offline
YOG_WRITE_OPENAPI=1 cargo test -p yog-api contract && (cd web && npm run gen:api)  # before the tests
(cd web && npm run typecheck)                                # the label the contract now asks for
cargo test --workspace                                       # DB-free; fixture suites and oracle included
cargo test -p yog-persistence --features integration-tests  # live Postgres: see CLAUDE.md
python3 crates/check-module-visibility.py                    # for a new platform's modules
```

Then see the sub-persistor write rows against a database: `/verify`, or `crates/README.md`
→ *Local development*.

## Definition of done

- [ ] Step 1 to 7: every bullet of the recipe's matching section done, the section re-read
- [ ] Every row of *What stays narrow* has the protocol's branch or line
- [ ] *Commands* green, and the sub-persistor seen writing rows
