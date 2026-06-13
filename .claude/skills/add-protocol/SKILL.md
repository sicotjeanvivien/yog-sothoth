---
name: add-protocol
description: Add a new Meteora/AMM protocol (or product) to Yog-Sothoth following the "voie 3" per-protocol pattern. Use when the user wants to support a new on-chain protocol/product end-to-end — new domain events, SQL tables, repositories, indexer sub-persistor, and optional API surface. Walks the exact recipe from crates/README.md and the three dispatch points that must change.
---

# Add a new protocol (voie 3)

Authoritative recipe: `crates/README.md` → *Adding a new protocol*. This skill is the
operational checklist with real file paths. Read the README section if anything below
is ambiguous — the README is the territory, this is the map.

The model is **typed per `(platform, protocol, event_kind)` all the way down**. Copy the
`meteora/damm_v2` implementation as your template — it is the complete, maintained
reference for every layer.

## Before you start — clarify scope

Confirm with the user (don't assume):
- **platform** (e.g. `meteora`), **product** (e.g. `dlmm`), and the snake_case `Protocol`
  string (e.g. `meteora_dlmm`).
- The on-chain **program ID** (base58).
- The **event kinds** to support (swap, liquidity, …) — each becomes its own
  module + table + repository + `persist_<kind>` method.

Note current state: `Protocol` (`crates/core/src/domain/protocol/model.rs`) already lists
`MeteoraDammV1` and `MeteoraDlmm`, but `DomainEvent` only has the `MeteoraDammV2` variant
and `ExtractionDispatcher` only routes DAMM v2 to a real extractor. So "adding a protocol"
usually means: add the `DomainEvent` outer variant + wire the dispatcher/persistor, and (if
it's a brand-new protocol not yet in the enum) add the `Protocol` variant + program ID.

## The three dispatch points (everything else is isolated per-protocol code)

1. `ExtrationDispacher::extract` — `crates/core/src/application/extraction/extraction_dispatcher.rs`
   (note the struct is spelled `ExtrationDispacher`). One new `match` branch + one field +
   one `::new()` in the constructor.
2. `EventPersistor::persist` — `crates/indexer/src/application/services/event_persistor.rs`.
   One new `DomainEvent::<NewProtocol>(e) => …` branch + one field.
3. `init_event_persistor` — `crates/indexer/src/bootstrap/daemon.rs`. One instantiation block
   wiring the new sub-persistor's repos + the shared `Arc<PoolMaintenance>`.

If you find yourself touching a fourth central registry, stop — you've left the pattern.

## Step 1 — `core` (no I/O, wasm-compatible; no Postgres/axum/HTTP here)

**Extraction side** — template: `crates/core/src/application/extraction/meteora/damm_v2/`
- Create `application/extraction/<platform>/<product>/` with:
  - `events.rs` — borsh wire-event mirrors
  - `extractor.rs` — walk inner instructions
  - `translator.rs` — wire → domain translation
- Create a top-level struct (e.g. `MeteoraDlmm`) implementing `EventExtractor`
  (`extract_events`). Register the module in `application/extraction/meteora.rs`.
- **Dispatch point 1**: add the branch in `ExtrationDispacher::extract` + the field + the
  `::new()` call.

**Domain side** — template: `crates/core/src/domain/meteora/damm_v2/`
- Per event kind, create `domain/<platform>/<product>/<event_kind>/` with `model.rs` and
  `repository.rs`. Prefix structs and cursors with the protocol
  (`MeteoraDlmmSwapEvent`, `MeteoraDlmmSwapCursor`).
- Add the sub-enum `<Platform><Product>Event` in `domain/<platform>/<product>.rs`, one
  variant per event kind.
- Add the outer variant in `DomainEvent` (`crates/core/src/domain/domain_event.rs`) and
  update **every** accessor: `pool_address`, `signature`, `timestamp`, `protocol`, `kind`.
- If the protocol is new to the enum: add the `Protocol` variant + program ID + the
  `all()` / `as_str()` / `program_id()` arms in `domain/protocol/model.rs`.

Keep domain types infra-neutral: `Pubkey` for addresses, `rust_decimal::Decimal` for
prices. Lossless `u128` becomes `BigDecimal` **only** at the persistence boundary — never
`sqlx::types` in `core`.

## Step 2 — `persistence` (no business logic)

- Add a forward-only migration `crates/persistence/migrations/NNN_<desc>.sql` (next number
  after the latest; never edit committed migrations). Create
  `<platform>_<product>_<event_kind>_events` tables — only protocol-relevant columns, no
  NULL columns for incompatible fields, no JSONB blob.
- In the **same migration**, add `GRANT INSERT, UPDATE ON <new_table> TO yog_indexer;`
  (SELECT is covered by default privileges in `setup_roles.sql`).
- Extend the cross-protocol VIEWs (`swap_events`, `liquidity_events`, …) with a new
  `UNION ALL` branch selecting from the new table with the `protocol` literal injected.
  Protocol-specific columns stay out of the VIEWs.
- Implement `Pg<Platform><Product><EventKind>EventRepository` under
  `crates/persistence/src/repositories/<platform>/<product>/<event_kind>/`, following the
  `Row + TryFrom<XxxRow> for XxxDomain` convention. Re-export from `lib.rs`.
- **Regenerate the SQLx cache** (mandatory — CI's `sqlx-check` fails otherwise):
  ```bash
  cd crates/persistence && cargo sqlx prepare
  ```
  Commit the updated `crates/persistence/.sqlx/`.

## Step 3 — `indexer` (no business logic, no SQL — wiring only)

- Create `crates/indexer/src/application/services/<platform>/<product>/event_persistor.rs`
  defining `<Platform><Product>EventPersistor`. It owns the per-event-kind repos (a
  `…Repos` bundle struct, see `DammV2Repos`) plus `Arc<PoolMaintenance>`. Its `persist`
  matches the protocol's sub-enum and dispatches to `persist_<kind>` methods.
- **Dispatch point 2**: add the `DomainEvent::<NewProtocol>(e)` branch in
  `EventPersistor::persist` + the field.
- **Dispatch point 3**: in `init_event_persistor` (`bootstrap/daemon.rs`) instantiate the
  repos bundle + the sub-persistor (reusing the shared `pool_maintenance`) and pass it to
  `EventPersistor::new`.

Respect **skip-and-log over abort-and-die**: per-event failures are logged + counted
(Prometheus) and stepped over; only loop-level failures bubble up.

## Step 4 — `api` (only when read access is needed)

- For new exposed event kinds, add a service under
  `crates/api/src/application/services/<platform>_<product>_<event_kind>_service.rs`.
- Add handlers + DTOs. Cross-protocol read surface → point at the VIEW; protocol-specific
  detail → point at the table directly. Reuse `ApiError` / `From<RepositoryError>`.
  Cursor pagination via `Page<T>`, default limit 50, hard cap 200. Pubkeys as base58,
  timestamps RFC3339.

## Step 5 — Tests

- Add fixture transactions under `crates/core/tests/fixtures/` (one per recognized
  signature for the new protocol) and integration tests in
  `crates/core/tests/live_detector.rs`.

## Verify (run from repo root)

```bash
cargo fmt --all
cargo clippy -p yog-api -p yog-core -p yog-context -p yog-indexer -p yog-persistence \
    --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# DB-backed repo tests (needs live Postgres):
cargo test -p yog-persistence --features integration-tests -- --include-ignored
```

Confirm the sub-persistor actually runs end-to-end against a DB before calling it done
(see the `/verify` skill or `crates/README.md` → *Local development*).

## Definition of done

- [ ] All three dispatch points updated (extract / persist / init)
- [ ] `DomainEvent` outer variant + all five accessors updated
- [ ] Migration created with `GRANT … TO yog_indexer` + VIEW `UNION ALL` branches
- [ ] `.sqlx/` regenerated and committed
- [ ] Repos re-exported from `persistence/lib.rs`
- [ ] Fixtures + `live_detector.rs` tests added
- [ ] fmt / clippy (-D warnings) / tests green
