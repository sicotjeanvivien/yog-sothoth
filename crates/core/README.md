# yog-core

Pure logic and domain types. No I/O, no runtime, no database — wasm-compatible by construction.

Every other crate depends on this one: it declares the domain entities, the repository traits that define every persistence contract, the event-extraction use case, and the AMM math. For the workspace-level picture (dependency graph, conventions, database roles, the add-a-protocol recipe), see [`crates/README.md`](../README.md).

---

## Layout

```
core/src/
├── domain/                ← entities + repository contracts
│   ├── meteora/damm_v2/   (one module per event kind — 19 today — each with
│   │                       model + repository trait; damm_v2.rs holds the
│   │                       MeteoraDammV2Event sub-enum; pool_properties/ holds
│   │                       this protocol's satellite payload)
│   ├── meteora/dlmm/      (pool_properties/ only — events land in v0.2.0)
│   ├── pool/              (Pool, PoolRepository — cross-protocol registry)
│   ├── pool_current_state/(CQRS projection of the latest per-pool state)
│   ├── pool_analytics/    (hourly aggregates read models)
│   ├── global_analytics/  (GlobalAnalytics — the /api/stats read model)
│   ├── signals/           (Signal, Severity, SignalDetector, EvalContext,
│   │                       SignalRepository + SignalFeed, DetectorError)
│   ├── swap_flow/         (PoolSwapFlow — directional volume read model)
│   ├── liquidity_flow/    (PoolLiquidityFlow — windowed add/remove + TVL read model)
│   ├── pool_price_snapshot/ (spot-vs-oracle read model)
│   ├── token_metadata/    (TokenMetadata + repo)
│   ├── token_price/       (TokenPrice + repo, PriceProvider)
│   ├── network_status/    (singleton snapshot)
│   ├── announcements/     (Announcement + AnnouncementLookup — operator → users
│   │                       banner; severity deliberately distinct from signals')
│   ├── watched_pool/      (allowlist)
│   ├── protocol/          (Protocol enum), trade_direction.rs, freshness_status/
│   ├── domain_event.rs    (two-level DomainEvent enum)
│   ├── pool_account/      (DecodedPoolAccount = PoolRegistryProperties for
│   │                       `pools` + two-level PoolAccountProperties for the
│   │                       satellite; PoolAccountResolver)
│   └── pool_properties/   (two-level PoolProperties + PoolPropertiesLookup —
│                           the read counterpart of pool_account/)
├── application/
│   ├── extraction/        ← transaction → domain events use case
│   │   ├── meteora/damm_v2/ (events.rs borsh mirrors, extractor.rs, translator.rs)
│   │   ├── anchor_event.rs  (generic Anchor event_cpi decoder)
│   │   ├── transaction_view.rs (TransactionView — the neutral input)
│   │   ├── rpc.rs           (JSON-RPC adapter; the only module naming a transport)
│   │   ├── event_extractor.rs / extraction_dispatcher.rs
│   │   └── outcome.rs       (ExtractionOutcome, ExtractionFailure)
│   └── decoder/           ← account bytes → pool properties use case
├── amm/                   ← pure AMM math (damm_v2.rs + dlmm.rs; common.rs is dormant)
├── tools/pagination.rs    ← Page<T>, Cursor enum
└── error/                 ← CoreError, RepositoryError, CoreResult<T>
```

File trees here are kept coarse on purpose — the module structure is the contract, the per-file detail lives in the code.

## Responsibilities

- **Domain models** (`domain/`) — entities and the repository traits behind every persistence contract. Per-protocol events live under `domain/<platform>/<product>/`; cross-protocol concepts (`Pool`, `PoolCurrentState`, `TokenPrice`, `Signal`, …) sit at the root of `domain/`. Read models used by a single consumer (e.g. `swap_flow` for the flow-imbalance detector, `global_analytics` for `/api/stats`) get their own slim module rather than widening an existing trait.
- **Two-level `DomainEvent`** (`domain/domain_event.rs`) — sum type with one outer variant per protocol, delegating to a sub-enum per event kind. `DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(...))` is the canonical shape. Accessors (`pool_address`, `signature`, `timestamp`, `protocol`, `kind`) delegate to the inner sub-enum.
- **Event extraction** (`application/extraction/`) — turns raw Solana transactions into protocol-agnostic `DomainEvent`s. Lives in `application/` rather than `domain/` because it orchestrates an external concern (the Solana transaction shape) into the domain language.
- **Account decoding** (`application/decoder/`) — the counterpart of the above: turns an on-chain account's raw bytes into a `DecodedPoolAccount`, the properties events never carry (mints, base fee, fee split, fee shape). The result is split by *who stores it* — `PoolRegistryProperties` for the cross-protocol `pools` registry, `PoolAccountProperties` for that protocol's satellite — each named by the table that owns it — so one read feeds two tables without either repository writing the other's. Dispatches on the account's owning program id — which is what the chain calls its `owner` — via `Protocol::from_program_id`, so callers need not know which protocol they asked for and one client can serve them all. Named for what it does, not for where the bytes came from: they may arrive over JSON-RPC today and over gRPC tomorrow, and nothing here changes. Two guards on every decode, neither redundant: the program id (by dispatch) and the account's Anchor discriminator. They are not defensive — at cp-amm's mint offsets a DLMM `LbPair` holds `reserve_x`/`reserve_y`, valid aligned `Pubkey`s, so an unguarded decode succeeds and writes vault addresses into mint columns. Failure is a typed `PoolAccountRejection`, not a bare `None`: in the only call path the caller asks for accounts of pools it queued itself, so *every* rejection signals a problem — an unindexed program, a missing decoder, the wrong account, or (the one to watch) a truncated account, the signature of an ABI change. `core` does no I/O, so it returns the reason and the caller logs and counts it — the same discipline as `ExtractionOutcome`.
- **Signals domain** (`domain/signals/`) — the write model (`Signal`, `Severity`) and the contracts of the signal engine: the `SignalDetector` trait (see below), the thin `EvalContext` (carries the tick clock, nothing else), `SignalRepository` (write + cooldown lookup) and `SignalFeed` (the API's read side: paginated feed, SSE delta reads, and the batched per-pool recent-signals lookup behind the pools-list indicator). The read model `SignalRecord { id, signal }` exists because the id only exists after insert — it never sits on the write-side `Signal`.
- **AMM math** (`amm/`) — `sqrt_price_to_price_a_in_b` (Q64.64 spot price) and the per-protocol fee conversions. `damm_v2::fee_numerator_to_bps` and `dlmm::base_fee_bps` both answer "what is this pool's base fee in bps", from different on-chain encodings — which is what lets `pools.fee_bps` mean one thing across protocols. `damm_v2::base_fee_numerator_at` answers a different question — "what is it charging *now*" — for the two fee shapes that decay over time: a port of cp-amm's own `fee_time_scheduler` and its Q64.64 `pow`, transcribed from the source rather than the docs, which give an approximate formula and omit both the before-activation branch and the scheduler's expiry.

  ⚠️ **Only one of the two saturates, and its cap is ours, not the chain's.** `dlmm::base_fee_bps` clamps at 1000 bps because its caller has nowhere to put an error (`PoolRegistryProperties::fee_bps` is not an `Option`, and an unresolvable pool would sit at the head of the enrichment queue forever); nothing is lost, since `base_factor` / `bin_step` / `base_fee_power_factor` are persisted raw beside the derived value. The clamp guards against an account lb_clmm should never have accepted — it is **not** a mirror of chain behaviour, which caps `base + variable` at *swap* time without normalising stored parameters.

  `damm_v2::fee_numerator_to_bps` does **not** clamp, and must not: cp-amm's ceiling is a function of the pool's `fee_version` — `MAX_FEE_BPS_V0` is 5000 (50 %) and `MAX_FEE_BPS_V1` is 9900 (99 %), while the 10 % figure is `MAX_FEE_NUMERATOR_POST_UPDATE`, the cap on an *operator update*, not on a pool. Clamping it to 1000 would report 10 % for a legitimate 50 % or 99 % anti-sniper launch pool — the exact pools a fee scheduler concerns and the dashboard highlights — with nothing to signal it: `fee_bps` is an unconstrained `NUMERIC` (baseline §015, deliberately), and the value stays plausible. `fee_numerator_to_bps(990_000_000) == 9900` is asserted in `amm/damm_v2_tests.rs` to keep this readable as a rule rather than an omission.

- **AMM formulas on the dormant path** (`amm/common.rs`, plus `damm_v2::fee_adjusted_amount` / `net_price_impact`) — reserve-ratio spot price, price impact, imbalance and fee-adjusted input. **No caller outside the group, on purpose**: they call each other, nothing else calls them. They model a constant-product AMM over vault totals, which DAMM v2 is not — and the resulting error has **no fixed sign**. In range, a concentrated position acts like a deeper pool than its real reserves suggest, so x·y=k *overstates* impact (17 % against a true 0.9 % on a plausible pool); out of range, vault reserves that back no trade inflate the depth and it *understates*. Which dominates is the pool's liquidity distribution, which these formulas do not have — so the output is neither a floor nor a ceiling. The correct formulation (`ΔA = L(1/√P − 1/√P_max)`) and the full trap are written up under *Détecteur Price impact creep* in the project tracker; each function also carries the warning at its own definition.

⚠️ Same definition, **different upper bound**. `dlmm::max_variable_fee_bps` computes how far a DLMM pool's volatility fee can climb above its floor, from parameters the satellite already stores. On real captured accounts that ceiling runs from ×1 to **×7 the floor**, so two pools at the same `fee_bps` are not interchangeable. The function exists so that claim is asserted against decoded accounts (`tests/pool_account_fixtures.rs`) rather than left as prose. Kept here because these formulas will eventually run in the browser via WASM. The borsh fee-blob decoders that used to sit here (`decode_base_fee_bps`, `decode_fee_config`, `decode_updated_base_fee_bps`, `decode_updated_dynamic_fee`) are **gone**: nothing reads a fee blob any more. Pool properties come from the on-chain account, decoded in `application/decoder/`, which is where layout decoding belongs.

  What stayed is `base_fee_kind_from`, which is *meant* to be here: it maps a `BaseFeeMode` discriminant and a period count to a `BaseFeeKind` — a rule, not a layout. A scheduler mode with zero periods is a constant fee, and mode 2 (rate limiter) must not consult the period count at all, because its variant reuses those bytes.
- **Pagination** (`tools/pagination.rs`) — `Page<T>` envelope and the discriminated `Cursor` enum used by every paginated repository method. A keyset cursor is only sound over an **immutable** sort key, and one listing sorts on a mutable one: `pools.last_seen_at`, rewritten on every event. That traversal is therefore anchored to a snapshot instant carried by the cursor (`PoolCursor::as_of`), which turns "the row moved across the cursor" into "the row left the result set" — the contract, and what it does *not* recover, is documented on `domain::PoolPage`. Adding a sort column means answering "is it immutable?", not "is it materialized?".
- **Transport indirection** (`application/extraction/rpc.rs`) — single point of contact for the JSON-RPC transaction types, and the only module of this crate that names one. It turns a `getTransaction` response into the neutral `TransactionView` and re-exports the types the ingestion binary needs. When the Solana SDK reshuffles those types, or when a second source arrives, this is where it lands.
- **Errors** (`error/`) — `CoreError` for domain-level failures, `RepositoryError` as the boundary type returned by every repository trait. Adapters convert their internal errors (e.g. `sqlx::Error`) into `RepositoryError` at their public surface.

## `EventExtractor` and `ExtractionDispatcher`

```rust
/// Per-protocol entry point. One implementation per supported protocol.
pub trait EventExtractor: Send + Sync {
    fn program_id(&self) -> Pubkey;
    fn extract_events(&self, tx: &TransactionView) -> CoreResult<ExtractionOutcome>;
}

/// Holds one pre-instantiated EventExtractor per protocol and routes
/// on the Protocol enum. yog-indexer depends on this, never on the
/// concrete extractors.
pub struct ExtractionDispatcher {
    damm_v2: MeteoraDammV2,
    damm_v1: MeteoraDammV1,   // stub — returns an empty outcome
    dlmm: MeteoraDlmm,        // stub — returns an empty outcome
}
```

The `Protocol` enum has three variants today, so the dispatcher has three
fields: a protocol reaches this match the moment it exists as a variant, well
before it extracts anything. `MeteoraDammV1` and `MeteoraDlmm` implement
`EventExtractor` and return `ExtractionOutcome::default()` — an empty outcome,
not an error, so a transaction of theirs is indexed as "nothing to record"
rather than counted as a failure.

The trait keeps the per-protocol contract explicit and testable; the enum dispatch is cheap — no `dyn` overhead, no allocation per transaction. `ExtractionDispatcher::extract` is one of the dispatch points a new protocol touches — `decode_pool_account` (`application/decoder.rs`) is this crate's other one (see the [add-a-protocol recipe](../README.md#adding-a-new-protocol)).

## `TransactionView` — the neutral input, and its adapters

`core` has no I/O, and it names no transport either. Extraction reads a
`TransactionView`: the coordinate that locates an event (`TransactionPosition` —
signature, block time, slot, and the transaction's index in its slot when the
source provides one) plus the ordered list of inner-instruction payloads, each
with the `Pubkey` of the program it was addressed to. Nothing else. A field
added here is a dependency the extractors gained on their source.

One **adapter** per source fills it. Today there is one, `extraction/rpc.rs`,
the only module of this crate that names `EncodedConfirmedTransactionWithStatusMeta`
— and it also re-exports the transport types `yog-indexer`'s fetcher needs,
because the encoding and the adapter are one contract: the fetcher must request
`JsonParsed`, since the adapter reads the `PartiallyDecoded` inner instructions
only that encoding produces. A Yellowstone gRPC source adds a *sibling adapter*,
not a second path through extraction; that is the whole point of the shape.

Two invariants every adapter owes, both documented on the type itself and
neither optional:

- **the order of `inner_instructions`** — a payload's position, after filtering
  on the emitting program, becomes the persisted `event_index`, part of the
  unique key of every event table. An adapter that reorders does not fail: it
  renumbers rows already stored, and a replay starts inserting duplicates;
- **which payloads it keeps** — permissively, everything it can represent,
  whatever program it targets. Deciding "is this an event" belongs to
  `decode_anchor_event_cpi` downstream. Narrowing shifts every event after the
  dropped one down by one, with the same silent effect. Only ever widen.

What guards them, and how far each guard reaches — because the two are not
witnessed by the same thing:

- `tests/extraction_oracle.rs` freezes the outcome of all 27 mainnet fixtures —
  events, `event_index`, unknowns, failures — against a committed witness.
  Reversing the instruction groups turns **6** of them red — the 6 whose cp-amm
  payloads actually span more than one group (`claim_position_fee`,
  `close_position`, `initialize_reward`, `lock_position`, `split_position2`,
  `swap_double`). The other 21 emit all their payloads from a single group, so
  no reordering of groups can be observed on them, whatever the corpus size;
- it does **not** witness the sort by group index, because every mainnet fixture
  already arrives in ascending order — delete `sort_by_key` and the whole suite
  stays green. That is what
  `rpc::tests::group_order_from_the_source_does_not_change_the_payload_order`
  is for: it hands the groups over reversed and fails without the sort.

Both mutations were run and their reach counted, not assumed. The count matters
as much as the red: `rotate_left(1)`, the first mutation tried, reddens only 2
fixtures — it moves group 0 to the end, so it is invisible unless group 0 itself
carries payloads. A mutation that reddens *something* proves less than one whose
blast radius you have measured.

## Anchor `event_cpi` extraction pipeline

Each Meteora program emits its events via Anchor's `emit_cpi!` mechanism — a self-CPI to an `event_authority` PDA, with a stable wire format:

```
[8 bytes EVENT_IX_TAG][8 bytes event discriminator][borsh payload]
```

where `EVENT_IX_TAG = sha256("anchor:event")[..8]` is the fixed prefix injected by Anchor. The pipeline runs in three stages:

```
whatever the source delivered
        ▼
[rpc.rs]                 from_rpc(tx) → TransactionView
        │                one adapter per source; the only place naming a transport
        ▼
[anchor_event.rs]        extract_anchor_event_cpis(view, program_id)
        │                keeps the payloads addressed to the program, in order;
        │                the position in that output becomes event_index
        ▼
[damm_v2/events.rs]      match discriminator → DammV2WireEvent, borsh-deserialize
        ▼
[damm_v2/translator.rs]  wire → domain, stamped with the view's position;
        │                fee_token_is_a from (collect_fee_mode, trade_direction).
        │                Self-contained — it never reads the transaction: mints
        │                are a pool property, resolved from the account by
        │                yog-context.
        ▼
ExtractionOutcome { events, unknown, failures }
```

Three failure types are distinguished in `ExtractionFailure` and counted as separate metric labels: `AnchorDecode` (prefix or payload-size mismatch), `Borsh` (schema mismatch), `Translation` (missing transferChecked context, invalid enum value).

## The `SignalDetector` trait

The signal engine's contract lives here so detectors depend only on `core` traits:

```rust
pub trait SignalDetector: Send + Sync {
    /// Stable snake_case tag, persisted verbatim as the `detector` column.
    fn name(&self) -> &'static str;
    /// Evaluation cadence — how often the engine ticks this detector.
    fn interval(&self) -> Duration;
    /// Rolling suppression window per (detector, pool); a higher severity
    /// overrides the suppression.
    fn cooldown(&self) -> Duration;
    /// Recompute from a DB snapshot — stateless between ticks.
    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError>;
}
```

Detectors are batch evaluators: they recompute from the database at each tick (the DB carries the state) and *return* candidate signals — the engine owns persistence and deduplication. See [`crates/signals/README.md`](../signals/README.md) for the runtime side.

## Repository traits

Each domain aggregate that needs persistence declares a repository trait in its module (`domain/<aggregate>/repository.rs`). Per-protocol event repositories follow the same pattern with protocol-prefixed types — `MeteoraDammV2SwapEventRepository` operates on `MeteoraDammV2SwapEvent` and `MeteoraDammV2SwapEventCursor`.

At runtime, the connected Postgres role determines which methods actually succeed: calling `insert` from the api process fails with `permission denied` from Postgres itself, by design (see [Database roles](../README.md#database-roles)). Where a trait's write side and read side have disjoint consumers, the trait is split per consumer — one lens per process, same `Pg*` struct behind both. The write/owning side keeps the `*Repository` name; read lenses are named by intent, from a deliberately small vocabulary:

- **`*Feed`** — a cursor-paginated, time-ordered listing (`SignalFeed`, `MeteoraDammV2SwapEventFeed`, `MeteoraDammV2LiquidityEventFeed`).
- **`*Lookup`** — point reads by key or of a projection (`TokenMetadataLookup`, `TokenPriceLookup`, `NetworkStatusLookup`, `PoolCurrentStateLookup`, `PoolPropertiesLookup`).
- **`PoolCatalog`** — the consultation surface of the pool registry (lookup + listing + counts).
- **`PoolAccountResolver`** — context's property-backfill lens, named by its capability.

Don't invent new vocabulary words for future lenses unless none of these fit.

## Conventions and invariants

Documented on the affected types and enforced at construction time:

- **`(token_a, token_b)` is the program's order, not a sort** — `token_a_mint` / `token_b_mint` hold the designation read off the on-chain account, and **nothing anywhere re-orders it**: measured on the local index, 50 of 136 pools have `token_a_mint > token_b_mint`. What it guarantees is internal consistency — amounts, reserves, the mint columns and the direction of `sqrt_price_to_price_a_in_b` all mean the same side — and stability over a pool's life. What it is *not* is a canonical pair key: deduplicating a pair across pools, or joining DAMM v2 to DLMM on "the same pair", needs a key built explicitly. Swap direction lives in the `TradeDirection` enum (`AtoB` | `BtoA`), read against this order.
- **No `protocol` field on per-protocol sub-events** — the protocol identity is encoded by the outer `DomainEvent` variant and by the SQL table name itself.
- **`fee_token_is_a` precomputed** — derived from `(collect_fee_mode, trade_direction)` in the translator, mirroring `cp-amm::FeeMode::get_fee_mode`.
- **Four fee components separated** — `claiming_fee`, `protocol_fee`, `compounding_fee`, `referral_fee` — so detectors can distinguish LP yield from protocol revenue.
- **Lossless `u128` in DB** — `next_sqrt_price` (Q64.64) and `liquidity_delta` are stored as `NUMERIC(39, 0)`; conversion happens in `persistence`, never here.
- **Off-chain decimal prices** — `TokenPrice::price_usd` is a `rust_decimal::Decimal` (infra-neutral, no `sqlx` leak). It carries one invariant the type cannot express on its own: a price must survive coercion to the column's `NUMERIC(38, 18)`, i.e. land in `[5e-19, 1e20)`. `TokenPrice::is_storable` states it (mirroring Postgres's round-half-away-from-zero), and `price_positivity.rs` asserts it agrees with the server at both ends. The test is deliberately *not* `> 0`: a sub-scale price is positive in Rust and becomes zero only on write — and a stored zero is far worse than an absent one, since it multiplies into a plausible number instead of NULL-annihilating and being caught by the `valuation_complete` guards. The two ends are enforced by *different* layers — the `CHECK` of migration 009 below, the column type itself above (`22003`, before any constraint runs) — which is why the Rust filter has to know both: a batch insert is aborted identically by either.
- **Integer signedness follows whoever owns the number** — on-chain quantities are unsigned (`u64` / `u128`), database-produced counts are `i64`. Decided 5 August 2026; the reasoning and the counter-argument are below.
- **Every event carries its position in the chain** — `slot`, `event_index` and `transaction_index` sit on all 19 DAMM v2 event types, assembled once per transaction as an `EventPosition` (see `domain/event_position.rs`) and threaded through the translators. `event_index` numbers the transaction's Anchor self-CPI payloads *including the ones we don't decode*, which is what lets `(signature, event_index, timestamp)` be a stable unique key: numbering only recognised events would renumber stored rows the day a new discriminator is implemented. The contract that guarantees it is the filter in `extract_anchor_event_cpis` — widen it freely, never narrow it (its doc-comment says why). `transaction_index` is `None` on the `getTransaction` path and exists for the gRPC migration.
- **An insert reports what it did** — event repositories return `InsertOutcome::{Inserted, Skipped}`, not `()`. `ON CONFLICT DO NOTHING` makes "no error" ambiguous, and discarding the difference is how a too-narrow unique key silently dropped 3–8 % of a pool's swaps until the August 2026 audit.

### Integer signedness: why counts are `i64`

On-chain quantities are **unsigned** — `amount_a: u64`, `slot: u64`,
`liquidity_delta: u128` — because the *chain* defines them that way. Carrying
them as `i64` would be a type-level lie, which is why `persistence` converts
through `convert_i64_to_u64` and raises `RepositoryError::Integrity` if a stored
value ever comes back negative.

Counts produced by the database are **`i64`**: `pools_priced`, `observed`,
`pool_count`, `swap_buckets_24h`, `swap_count`. They have no upstream unsigned
truth to preserve — they are born from `COUNT(*)`, which is a `BIGINT`, and
Postgres has no unsigned integer type at all. `u64` would not make them *truer*,
only narrower. Database identities (`id: i64`, from `BIGSERIAL`) follow for the
same reason.

Row types in `persistence` are `i64` regardless, and that part is not a choice:
there is no `Decode<Postgres> for u64`.

⚠️ **The counter-argument is real and was weighed.** An `i64` count arguably
lets the storage type define the domain, which is what the infra-neutrality rule
above forbids in spirit. It was decided against on **5 August 2026** (audit
PR #106): no known defect would have been prevented, the values are bounded by
row counts far below `i64::MAX`, and the "never negative" invariant is already
enforced where it can actually be violated — at the wire boundary, by the web's
`z.number().int().nonnegative()`. Switching would be all nine counters or none;
anything partial puts two conventions in one struct.

## Tests

```bash
cargo test -p yog-core                      # unit + fixture tests
cargo test -p yog-core extraction           # extraction only
```

Fixture transactions for the extraction pipeline live under `core/tests/fixtures/` — one real mainnet transaction per recognized event kind.

### Real pool accounts, and why they earn their place

`tests/fixtures/<protocol>/accounts/` holds real pool accounts as raw base64,
exercised by `tests/pool_account_fixtures.rs`: eleven cp-amm `Pool` accounts
covering every `BaseFeeMode` the program defines and both dynamic-fee values,
and nine DLMM `LbPair` accounts spanning `bin_step` 1..=400 and `base_factor`
0..=40 000 — including a zero-fee pool and pools with and without a dynamic fee.

The DLMM set was not hand-picked: it comes from resolving every account key in
the DLMM transaction fixtures and keeping those owned by lb_clmm at 904 bytes,
so it is drawn from pools we have actually seen.

They exist because **the decoder's synthetic tests cannot catch a wrong offset**:
they build the buffer with the same constants the code reads it with, so both
agree on a lie. Measured, by moving `BASE_FEE_MODE_OFFSET` one byte: the fourteen
synthetic tests stayed green, the real accounts failed immediately. That is the
gap `partner_fee_percent` lived in for months.

The expectations are in the test file, not in the fixtures — the fixtures hold
bytes and provenance only, so reviewing the test means reviewing what we claim
the bytes mean. How each was established (a realized fee rate computed from swap
events, mints resolving to USDC/SOL) is documented in the module.

Recapturing is one `getMultipleAccounts` call, spelled out in that module doc. A
fixture is a snapshot: a program upgrade that moves a field turns these red, and
that is the intended channel for learning about it.

**One gap is stated rather than hidden.** DLMM's `base_fee_power_factor` is 0 on
every account reachable from our fixtures, so its offset is *consistent with* the
layout but unwitnessed — every fixture would decode identically if the field were
ignored. That is the shape `partner_fee_percent` had. The difference is that
lb_clmm names this one and Meteora's formula documents it, so it is a real field
with no live user rather than a padding byte read by mistake; the arithmetic is
unit-tested on non-zero values, only the *offset* is unproven. A test asserts the
gap still exists, so capturing a pool with a non-zero power factor turns it red
and prompts a direct assertion instead.

## Compilation targets

- `cargo build` → native library, linked into every binary ✅
- `wasm-pack build` → WASM module for the browser 🚧 deferred — reassessed at v0.3 — auth (see [`crates/README.md`](../README.md#wasm-yog-wasm))
