---
name: add-protocol
description: Add a new Meteora/AMM protocol (or product) to Yog-Sothoth following the "voie 3" per-protocol pattern. Use when the user wants to support a new on-chain protocol/product end-to-end — new domain events, SQL tables, repositories, indexer sub-persistor, and optional API surface. Walks the exact recipe from crates/README.md and the seven dispatch points that must change.
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
`MeteoraDlmm`, and DLMM's account side is done — its `LbPair` decoder, its satellite
`meteora_dlmm_pool_properties` and its resolver in `context`. What it lacks is events:
`DomainEvent` only has the `MeteoraDammV2` variant and `MeteoraDlmm::extract_events` is a
stub. So finishing DLMM means the extraction, domain-event, event-table and sub-persistor
steps below, **flipping `MeteoraDlmm::is_implemented` to `true`** in the same change (until
then the ingestion never subscribes to the program and every test stays green on zero
events), and giving its fixtures an oracle — `extraction_oracle_tests.rs` reads
`fixtures/damm_v2` only and asserts its exact count. A brand-new protocol also needs the
`Protocol` variant + program ID and the whole account side.

## The dispatch points (everything else is isolated per-protocol code)

In the order of the table in `crates/README.md` → *What stays narrow*:

1. `ExtractionDispatcher::extract` —
   `crates/core/src/application/extraction/extraction_dispatcher.rs`. One new `match` branch
   + one field + one `::new()` in the constructor.
2. `ExtractionDispatcher::implemented_protocols` — same file, one branch.
   ⚠️ It decides whether the ingestion **subscribes to the program id**.
   Its answer comes from `EventExtractor::is_implemented`, which has no default: say `false`
   while the extractor is a stub, or the indexer pays a firehose — every transaction of the
   program fetched or streamed, decoded and discarded — for zero rows.
3. `decode_pool_account` — `crates/core/src/application/decoder.rs`. One branch routing the
   new `Protocol` to its account decoder.
4. `EventPersistor::persist` — `crates/indexer/src/application/services/event_persistor.rs`.
   One new `DomainEvent::<NewProtocol>(e) => …` branch + one field.
5. `init_event_persistor` — `crates/indexer/src/bootstrap/daemon/init.rs`. One instantiation block
   wiring the new sub-persistor's repos + the shared `Arc<PoolMaintenance>`.
6. The `pool_account_resolvers` vec — `crates/context/src/bootstrap/daemon.rs`. One line
   pushing the new `Pg*` resolver; `PoolAccountWorker` names no protocol.
7. The `PoolProperties` match — `crates/api/src/http/dto/response/pool.rs`. One optional
   field named after the protocol; the `match` is exhaustive with no wildcard arm, so the
   compiler asks.

If you find yourself touching a central registry outside this list, stop — you've left
the pattern.

## Step 1 — `core` (no I/O, wasm-compatible; no Postgres/axum/HTTP here)

**Extraction side** — template: `crates/core/src/application/extraction/meteora/damm_v2/`
- Create `application/extraction/<platform>/<product>/` with:
  - `events.rs` — borsh wire-event mirrors
  - `extractor.rs` — walk inner instructions
  - `translator.rs` — wire → domain translation
- Create a top-level struct (e.g. `MeteoraDlmm`) implementing `EventExtractor`
  (`extract_events` **and** `is_implemented` — the compiler asks, since it has no default).
  Register the module in `application/extraction/meteora.rs`.
- **Dispatch points 1 and 2**: add the branch in `ExtractionDispatcher::extract`, the branch
  in `::implemented_protocols`, the field, and the `::new()` call.

**Domain side** — template: `crates/core/src/domain/meteora/damm_v2/`
- Per event kind, create `domain/<platform>/<product>/<event_kind>/` with `model.rs` and
  `repository.rs`. Prefix structs and cursors with the protocol
  (`MeteoraDlmmSwapEvent`, `MeteoraDlmmSwapEventCursor`).
- Add the sub-enum `<Platform><Product>Event` in `domain/<platform>/<product>.rs`, one
  variant per event kind.
- Add the outer variant in `DomainEvent` (`crates/core/src/domain/domain_event.rs`) and
  update **every** accessor: `pool_address`, `signature`, `timestamp`, `protocol`, `kind`.
- If the protocol is new to the enum: add the `Protocol` variant + program ID + the
  `all()` / `as_str()` / `program_id()` arms in `domain/protocol/model.rs` — **and the
  `FromStr` arm**, which ends in a `_ => Err(…)` wildcard, so the compiler will not ask:
  forget it and every persistence read of the new protocol's rows fails at runtime.

**Account side** — the properties events never carry (mints, base fee, fee shape);
template: `crates/core/src/application/decoder/meteora/`
- Decode the pool account in `application/decoder/<platform>/<product>.rs`, returning a
  `DecodedPoolAccount`, from the account bytes alone (`decode_pool_account(data)`, like the
  two templates). It guards on the Anchor discriminator; the program id is guarded by the
  dispatch, which reaches it only through `Protocol::from_program_id` — neither
  is redundant.
- **Dispatch point 3**: the branch in `decode_pool_account`.
- Add the matching variants to `PoolAccountProperties` (write side, `domain/pool_account/`)
  and `PoolProperties` (read side, `domain/pool_properties/`).
- Ground the decoder on **real mainnet accounts** before trusting it — fixtures under
  `crates/core/tests/fixtures/<product>/accounts/` (`damm_v2/`, `dlmm/` — the product, not
  the `Protocol` string). A synthetic buffer agrees with the
  decoder on a wrong offset.

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
- Add the **pool-properties satellite** `<platform>_<product>_pool_properties`: one row per
  pool, primary-keyed on `pool_address`. Copy the shape from `001_baseline.sql` §9, the DLMM
  satellite — the generated `protocol` column plus the composite
  `FOREIGN KEY (pool_address, protocol) REFERENCES pools (pool_address, protocol) ON DELETE
  CASCADE`. Grant it
  to `yog_context` (its sole writer), not to `yog_indexer`.
- Add every new table's line to the privilege matrix in
  `crates/persistence/tests/privileges.rs` — it asserts in both directions.
- Extend the cross-protocol VIEWs (`swap_events`, `liquidity_events`, …) with a new
  `UNION ALL` branch selecting from the new table with the `protocol` literal injected.
  Protocol-specific columns stay out of the VIEWs.
- Implement `Pg<Platform><Product><EventKind>EventRepository` under
  `crates/persistence/src/repositories/<platform>/<product>/<event_kind>/`, following the
  `Row + TryFrom<XxxRow> for XxxDomain` convention. Re-export from `lib.rs`.
- Implement the satellite's `PoolAccountResolver` (write + queue) and
  `PoolPropertiesLookup` (read) on one `Pg*` struct. ⚠️ Its `list_unresolved` **must**
  filter on its own protocol, or the queue starves behind other protocols' pools.
- **Regenerate the SQLx cache** (mandatory — CI's `sqlx-check` fails otherwise):
  ```bash
  cd crates/persistence && cargo sqlx prepare -- --all-targets --all-features
  ```
  Commit the updated `crates/persistence/.sqlx/`.

## Step 3 — `indexer` (no business logic, no SQL — wiring only)

- Create `crates/indexer/src/application/services/<platform>/<product>/event_persistor.rs`
  defining `<Platform><Product>EventPersistor`. It owns the per-event-kind repos (a
  `…Repos` bundle struct, see `DammV2Repos`) plus `Arc<PoolMaintenance>`. Its `persist`
  matches the protocol's sub-enum and dispatches to `persist_<kind>` methods.
- **Dispatch point 4**: add the `DomainEvent::<NewProtocol>(e)` branch in
  `EventPersistor::persist` + the field.
- **Dispatch point 5**: in `init_event_persistor` (`bootstrap/daemon/init.rs`) instantiate the
  repos bundle + the sub-persistor (reusing the shared `pool_maintenance`) and pass it to
  `EventPersistor::new`.

Respect **skip-and-log over abort-and-die**: per-event failures are logged + counted
(Prometheus) and stepped over; only loop-level failures bubble up.

## Step 4 — `context`

- **Dispatch point 6**: push the new `Pg*` resolver into the `pool_account_resolvers` vec
  (`crates/context/src/bootstrap/daemon.rs`). That is the whole wiring.

## Step 5 — `api` (only when read access is needed)

- **Dispatch point 7**: the protocol's block in `crates/api/src/http/dto/response/pool.rs`
  (compiler-forced once the `PoolProperties` variant exists).
- For new exposed event kinds, add a service under
  `crates/api/src/application/services/<platform>/<product>/` (template:
  `meteora/damm_v2/swap.rs`) — only when its repository, params and result are irreducibly
  that product's; cross-protocol services stay at the root and name no protocol.
- Add handlers + DTOs. Cross-protocol read surface → point at the VIEW; protocol-specific
  detail → point at the table directly. Reuse `ApiError` / `From<RepositoryError>`.
  Cursor pagination via `Page<T>`, default limit 50, hard cap 200. Pubkeys as base58,
  timestamps RFC3339.

## Step 6 — Tests

- Add fixture transactions under `crates/core/tests/fixtures/` (one per recognized
  signature for the new protocol) and extraction tests in
  `crates/indexer/src/infra/rpc/tests/fixture_pipeline_tests.rs` — one directory
  over from the adapter that turns a fixture into an `OnChainTransaction`, the
  fixtures stay in `yog-core`. Plus the account fixtures of step 1 and the
  privilege-matrix lines of step 2.

## Verify (run from repo root)

```bash
cargo fmt --all
cargo clippy -p yog-api -p yog-archive -p yog-bootstrap -p yog-core -p yog-context \
    -p yog-indexer -p yog-persistence -p yog-signals \
    --all-targets --all-features -- -D warnings
cargo test --workspace                     # DB-free
cargo test -p yog-indexer                  # the fixture suites and the extraction oracle
# DB-backed repo tests (need a live Postgres, admin DATABASE_URL — see CLAUDE.md).
# `cargo test --workspace --all-features` would run these too: it is not DB-free.
cargo test -p yog-persistence --features integration-tests
```

Confirm the sub-persistor actually runs end-to-end against a DB before calling it done
(see the `/verify` skill or `crates/README.md` → *Local development*).

## Definition of done

- [ ] All seven dispatch points updated (extract / implemented_protocols /
      decode_pool_account / persist / init / pool_account_resolvers / PoolProperties match)
- [ ] `is_implemented` answers `true` once the extractor really extracts
- [ ] Account decoder grounded on real mainnet accounts; satellite table + resolver
- [ ] `DomainEvent` outer variant + all five accessors updated
- [ ] Migration created with `GRANT … TO yog_indexer` + VIEW `UNION ALL` branches
- [ ] Every new table in the `tests/privileges.rs` matrix
- [ ] `.sqlx/` regenerated and committed
- [ ] Repos re-exported from `persistence/lib.rs`
- [ ] Fixtures added in `yog-core`, extraction tests in `yog-indexer`
- [ ] fmt / clippy (-D warnings) / tests green
