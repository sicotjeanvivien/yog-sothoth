//! Per-pool projection of the latest observed on-chain state.
//!
//! [`PoolCurrentState`] is a read model maintained by the indexer: every swap
//! or liquidity event triggers an upsert that brings this struct in sync with
//! what was just persisted in the append-only event tables.
//!
//! The domain types here are deliberately decoupled from any persistence
//! detail (no sqlx attributes, no Postgres types). Conversions to/from the
//! database row live in `crates/persistence/src/repositories/pool_current_state.rs`.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::{EventPosition, MeteoraDammV2LiquidityEventKind, Protocol};

/// Kind of the most recent event that touched a pool.
///
/// Mirrors the `last_event_kind` CHECK constraint in
/// `003_pool_current_state.sql`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LastEventKind {
    Swap,
    LiquidityAdd,
    LiquidityRemove,
}

impl LastEventKind {
    /// Wire string used in the database and in HTTP responses.
    pub fn as_str(self) -> &'static str {
        match self {
            LastEventKind::Swap => "swap",
            LastEventKind::LiquidityAdd => "liquidity_add",
            LastEventKind::LiquidityRemove => "liquidity_remove",
        }
    }

    /// Parse the wire string. Returns `None` for unknown variants — the caller
    /// is expected to surface this as a data-integrity error since the SQL
    /// CHECK constraint forbids storing anything else.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "swap" => Some(LastEventKind::Swap),
            "liquidity_add" => Some(LastEventKind::LiquidityAdd),
            "liquidity_remove" => Some(LastEventKind::LiquidityRemove),
            _ => None,
        }
    }
}

impl std::fmt::Display for LastEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bridge from the liquidity-event domain enum to the projection event kind.
///
/// Lives here rather than on `MeteoraDammV2LiquidityEventKind` to keep the latter unaware
/// of the projection: the projection depends on the event domain, not the
/// other way around.
impl From<MeteoraDammV2LiquidityEventKind> for LastEventKind {
    fn from(kind: MeteoraDammV2LiquidityEventKind) -> Self {
        match kind {
            MeteoraDammV2LiquidityEventKind::Add => LastEventKind::LiquidityAdd,
            MeteoraDammV2LiquidityEventKind::Remove => LastEventKind::LiquidityRemove,
        }
    }
}

/// Latest known state of a pool, materialized from the event stream.
///
/// Field ordering follows the SQL column ordering in
/// `003_pool_current_state.sql` for ease of cross-reference.
///
/// * `reserve_a` / `reserve_b` are u64 in the protocol's canonical
///   (token_a, token_b) order; on the wire they map to `BIGINT`, matching
///   the upstream `swap_events` / `liquidity_events` hypertables.
/// * `last_sqrt_price` is `None` until the first swap is observed
///   (Q64.64 fixed-point as u128, stored as `NUMERIC(39, 0)`).
/// * `liquidity` is `None` until the first liquidity event is observed
///   (concentrated-liquidity L as u128, stored as `NUMERIC(39, 0)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCurrentState {
    pub pool_address: Pubkey,
    pub protocol: Protocol,

    pub last_event_at: DateTime<Utc>,
    pub last_event_kind: LastEventKind,
    pub last_signature: Signature,

    pub reserve_a: u64,
    pub reserve_b: u64,

    pub last_sqrt_price: Option<u128>,
    pub last_swap_at: Option<DateTime<Utc>>,

    pub liquidity: Option<u128>,
    pub last_liquidity_at: Option<DateTime<Utc>>,

    pub updated_at: DateTime<Utc>,
}

/// Payload describing a state change to apply via
/// [`PoolCurrentStateRepository::upsert`].
///
/// Constructed by the indexer from the event it just persisted. Unlike
/// [`PoolCurrentState`], this struct carries only what the event provides —
/// e.g. a swap-derived upsert sets `sqrt_price` but leaves `liquidity` as
/// `None` (existing value is preserved by the repository).
///
/// See the repository contract for the merge semantics and the stale-write
/// guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCurrentStateUpsert {
    pub pool_address: Pubkey,
    pub protocol: Protocol,

    /// Where the source event sits in the chain. It carries the signature and
    /// the timestamp the projection displays, **and** the `(slot,
    /// transaction_index, event_index)` the repository orders on — the two
    /// used to be separate fields, and ordering on the timestamp alone
    /// rejected a third of all updates (they share a second).
    pub position: EventPosition,
    pub event_kind: LastEventKind,

    pub reserve_a: u64,
    pub reserve_b: u64,

    /// Set only when the upsert originates from a swap event.
    pub sqrt_price: Option<u128>,

    /// Set only when the upsert originates from a liquidity event.
    pub liquidity: Option<u128>,
}

impl PoolCurrentStateUpsert {
    /// Build an upsert payload from a swap event.
    pub fn from_swap(
        pool_address: Pubkey,
        protocol: Protocol,
        position: EventPosition,
        reserve_a: u64,
        reserve_b: u64,
        sqrt_price: u128,
    ) -> Self {
        Self {
            pool_address,
            protocol,
            position,
            event_kind: LastEventKind::Swap,
            reserve_a,
            reserve_b,
            sqrt_price: Some(sqrt_price),
            liquidity: None,
        }
    }

    /// Build an upsert payload from a liquidity event.
    ///
    /// `kind` is the domain enum; its mapping to the projection event kind
    /// goes through the [`From<MeteoraDammV2LiquidityEventKind> for LastEventKind`] impl
    /// defined above so add/remove sourcing stays in one place.
    pub fn from_liquidity(
        pool_address: Pubkey,
        protocol: Protocol,
        position: EventPosition,
        kind: MeteoraDammV2LiquidityEventKind,
        reserve_a: u64,
        reserve_b: u64,
        liquidity: u128,
    ) -> Self {
        Self {
            pool_address,
            protocol,
            position,
            event_kind: kind.into(),
            reserve_a,
            reserve_b,
            sqrt_price: None,
            liquidity: Some(liquidity),
        }
    }
}

/// What an upsert did, and what it could not know.
///
/// Richer than a boolean because the ordering key is **partial**:
/// `transaction_index` is empty on the `getTransaction` ingestion path, so two
/// transactions landing in the same slot and touching the same pool are ranked
/// on `event_index` alone — an ordinal that numbers the emissions of *one*
/// transaction, so comparing it across two ranks unlike things. The state
/// converges to the largest index, which favours a leg deep inside a routed
/// transaction over a single-leg swap of the same block.
///
/// Rather than let that pass for healthy concurrency (the mistake the label
/// `pool_current_state_stale` made for months), the repository reports when it
/// happened and the caller counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCurrentStateUpsertOutcome {
    /// `true` when the row was written (inserted or updated), `false` when the
    /// ordering guard suppressed it.
    pub applied: bool,

    /// The state this upsert met came from the **same slot** under a
    /// **different signature** — the one case the reachable key cannot order.
    ///
    /// Reported whether the upsert applied or not, on purpose: an ambiguity
    /// that wrongly *accepts* (overwriting newer state with older) does as
    /// much damage as one that wrongly rejects, and a counter that only saw
    /// rejections would be a lower bound dressed up as a measurement.
    ///
    /// It is a lower bound anyway, for a second reason the implementation
    /// documents: under concurrent writers the report and the guard do not
    /// read the same row version, and the miss goes in the direction that
    /// flatters the assumption. Expect it to be large on hot pools — the
    /// signal is its ratio to applied upserts, not its absolute value.
    pub same_slot_ambiguity: bool,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
