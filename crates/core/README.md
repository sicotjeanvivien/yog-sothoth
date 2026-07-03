# yog-core

Pure logic and domain types. No I/O, no runtime, no database — wasm-compatible by construction.

Every other crate depends on this one: it declares the domain entities, the repository traits that define every persistence contract, the event-extraction use case, and the AMM math. For the workspace-level picture (dependency graph, conventions, database roles, the add-a-protocol recipe), see [`crates/README.md`](../README.md).

---

## Layout

```
core/src/
├── domain/                ← entities + repository contracts
│   ├── meteora/damm_v2/   (one module per event kind — 11 today — each with
│   │                       model + repository trait; damm_v2.rs holds the
│   │                       MeteoraDammV2Event sub-enum)
│   ├── pool/              (Pool, PoolRepository — cross-protocol registry)
│   ├── pool_current_state/(CQRS projection of the latest per-pool state)
│   ├── pool_analytics/    (hourly aggregates read models)
│   ├── global_analytics/  (GlobalAnalytics — the /api/stats read model)
│   ├── signals/           (Signal, Severity, SignalDetector, EvalContext,
│   │                       SignalRepository + SignalFeedRepository, DetectorError)
│   ├── swap_flow/         (PoolSwapFlow — directional volume read model)
│   ├── pool_price_snapshot/ (spot-vs-oracle read model)
│   ├── token_metadata/    (TokenMetadata + repo)
│   ├── token_price/       (TokenPrice + repo, PriceProvider)
│   ├── network_status/    (singleton snapshot)
│   ├── watched_pool/      (allowlist)
│   ├── protocol/          (Protocol enum), trade_direction.rs, freshness_status/
│   └── domain_event.rs    (two-level DomainEvent enum)
├── application/
│   └── extraction/        ← transaction → domain events use case
│       ├── meteora/damm_v2/ (events.rs borsh mirrors, extractor.rs, translator.rs)
│       ├── anchor_event.rs  (generic Anchor event_cpi decoder)
│       ├── event_extractor.rs / extraction_dispatcher.rs
│       └── outcome.rs       (ExtractionOutcome, ExtractionFailure)
├── amm/                   ← pure AMM math (common.rs + damm_v2.rs)
├── tools/pagination.rs    ← Page<T>, Cursor enum
├── error/                 ← CoreError, RepositoryError, CoreResult<T>
└── solana_types.rs        ← re-export hub for Solana SDK types
```

File trees here are kept coarse on purpose — the module structure is the contract, the per-file detail lives in the code.

## Responsibilities

- **Domain models** (`domain/`) — entities and the repository traits behind every persistence contract. Per-protocol events live under `domain/<platform>/<product>/`; cross-protocol concepts (`Pool`, `PoolCurrentState`, `TokenPrice`, `Signal`, …) sit at the root of `domain/`. Read models used by a single consumer (e.g. `swap_flow` for the flow-imbalance detector, `global_analytics` for `/api/stats`) get their own slim module rather than widening an existing trait.
- **Two-level `DomainEvent`** (`domain/domain_event.rs`) — sum type with one outer variant per protocol, delegating to a sub-enum per event kind. `DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(...))` is the canonical shape. Accessors (`pool_address`, `signature`, `timestamp`, `protocol`, `kind`) delegate to the inner sub-enum.
- **Event extraction** (`application/extraction/`) — turns raw Solana transactions into protocol-agnostic `DomainEvent`s. Lives in `application/` rather than `domain/` because it orchestrates an external concern (the Solana transaction shape) into the domain language.
- **Signals domain** (`domain/signals/`) — the write model (`Signal`, `Severity`) and the contracts of the signal engine: the `SignalDetector` trait (see below), the thin `EvalContext` (carries the tick clock, nothing else), `SignalRepository` (write + cooldown lookup) and `SignalFeedRepository` (read side used by the API feed). The read model `SignalRecord { id, signal }` exists because the id only exists after insert — it never sits on the write-side `Signal`.
- **AMM math** (`amm/`) — price, reserves, slippage, imbalance formulas, plus DAMM v2-specific decoding: `sqrt_price_to_price_a_in_b` (Q64.64 spot price) and the base-fee decoders (`decode_base_fee_bps`, `decode_updated_base_fee_bps`) used on raw on-chain fee bytes. Kept here because these formulas will eventually run in the browser via WASM.
- **Pagination** (`tools/pagination.rs`) — `Page<T>` envelope and the discriminated `Cursor` enum used by every paginated repository method.
- **Solana SDK indirection** (`solana_types.rs`) — single point of contact for types reshuffled by Solana SDK releases. When the SDK restructures, only this file changes.
- **Errors** (`error/`) — `CoreError` for domain-level failures, `RepositoryError` as the boundary type returned by every repository trait. Adapters convert their internal errors (e.g. `sqlx::Error`) into `RepositoryError` at their public surface.

## `EventExtractor` and `ExtractionDispatcher`

```rust
/// Per-protocol entry point. One implementation per supported protocol.
pub trait EventExtractor: Send + Sync {
    fn program_id(&self) -> &str;
    fn extract_events(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome>;
}

/// Holds one pre-instantiated EventExtractor per protocol and routes
/// on the Protocol enum. yog-indexer depends on this, never on the
/// concrete extractors.
pub struct ExtractionDispatcher {
    damm_v2: MeteoraDammV2,
    // future: damm_v1, dlmm, raydium_clmm, ...
}
```

The trait keeps the per-protocol contract explicit and testable; the enum dispatch is cheap — no `dyn` overhead, no allocation per transaction. `ExtractionDispatcher::extract` is one of the three dispatch points a new protocol touches (see the [add-a-protocol recipe](../README.md#adding-a-new-protocol)).

## Anchor `event_cpi` extraction pipeline

Each Meteora program emits its events via Anchor's `emit_cpi!` mechanism — a self-CPI to an `event_authority` PDA, with a stable wire format:

```
[8 bytes EVENT_IX_TAG][8 bytes event discriminator][borsh payload]
```

where `EVENT_IX_TAG = sha256("anchor:event")[..8]` is the fixed prefix injected by Anchor. The pipeline runs in three stages:

```
EncodedConfirmedTransactionWithStatusMeta
        ▼
[anchor_event.rs]        extract_anchor_event_cpis(tx, program_id)
        │                iterates inner_instructions, filters on programId +
        │                EVENT_IX_TAG, returns decoded base58 payloads
        ▼
[damm_v2/events.rs]      match discriminator → DammV2WireEvent, borsh-deserialize
        ▼
[damm_v2/translator.rs]  wire → domain: mints from surrounding transferChecked,
        │                fee_token_is_a from (collect_fee_mode, trade_direction)
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

Each domain aggregate that needs persistence declares a repository trait in its module (`domain/<aggregate>/repository.rs`). Per-protocol event repositories follow the same pattern with protocol-prefixed types — `MeteoraDammV2SwapEventRepository` operates on `MeteoraDammV2SwapEvent` and `MeteoraDammV2SwapCursor`.

At runtime, the connected Postgres role determines which methods actually succeed: calling `insert` from the api process fails with `permission denied` from Postgres itself, by design (see [Database roles](../README.md#database-roles)). Where a trait's write side and read side have disjoint consumers, the trait is split per consumer (e.g. `PoolAccountResolver` vs the rest of `PoolRepository`, `SignalRepository` vs `SignalFeedRepository`) — one lens per process, same `Pg*` struct behind both.

## Conventions and invariants

Documented on the affected types and enforced at construction time:

- **Mints sorted by raw bytes** — in `Pool` and DAMM v2 swap/liquidity events, `token_a_mint` / `token_b_mint` are ordered by `Pubkey::Ord`. Stable regardless of swap direction; differs from the Meteora SDK canonical convention.
- **Canonical `(token_a, token_b)` exposure** — amounts and reserves are exposed in canonical order; swap direction lives in the `TradeDirection` enum (`AtoB` | `BtoA`).
- **No `protocol` field on per-protocol sub-events** — the protocol identity is encoded by the outer `DomainEvent` variant and by the SQL table name itself.
- **`fee_token_is_a` precomputed** — derived from `(collect_fee_mode, trade_direction)` in the translator, mirroring `cp-amm::FeeMode::get_fee_mode`.
- **Four fee components separated** — `claiming_fee`, `protocol_fee`, `compounding_fee`, `referral_fee` — so detectors can distinguish LP yield from protocol revenue.
- **Lossless `u128` in DB** — `next_sqrt_price` (Q64.64) and `liquidity_delta` are stored as `NUMERIC(39, 0)`; conversion happens in `persistence`, never here.
- **Off-chain decimal prices** — `TokenPrice::price_usd` is a `rust_decimal::Decimal` (infra-neutral, no `sqlx` leak).

## Tests

```bash
cargo test -p yog-core                      # unit + fixture tests
cargo test -p yog-core extraction           # extraction only
```

Fixture transactions for the extraction pipeline live under `core/tests/fixtures/` — one real mainnet transaction per recognized event kind.

## Compilation targets

- `cargo build` → native library, linked into every binary ✅
- `wasm-pack build` → WASM module for the browser 🚧 deferred — reassessed at v0.2 (see [`crates/README.md`](../README.md#wasm-yog-wasm))
