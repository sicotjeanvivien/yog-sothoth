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

use crate::domain::LiquidityEventKind;

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
/// Lives here rather than on `LiquidityEventKind` to keep the latter unaware
/// of the projection: the projection depends on the event domain, not the
/// other way around.
impl From<LiquidityEventKind> for LastEventKind {
    fn from(kind: LiquidityEventKind) -> Self {
        match kind {
            LiquidityEventKind::Add => LastEventKind::LiquidityAdd,
            LiquidityEventKind::Remove => LastEventKind::LiquidityRemove,
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
    pub pool_address: String,
    pub protocol: String,

    pub last_event_at: DateTime<Utc>,
    pub last_event_kind: LastEventKind,
    pub last_signature: String,

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
    pub pool_address: String,
    pub protocol: String,

    pub event_at: DateTime<Utc>,
    pub event_kind: LastEventKind,
    pub signature: String,

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
        pool_address: impl Into<String>,
        protocol: impl Into<String>,
        event_at: DateTime<Utc>,
        signature: impl Into<String>,
        reserve_a: u64,
        reserve_b: u64,
        sqrt_price: u128,
    ) -> Self {
        Self {
            pool_address: pool_address.into(),
            protocol: protocol.into(),
            event_at,
            event_kind: LastEventKind::Swap,
            signature: signature.into(),
            reserve_a,
            reserve_b,
            sqrt_price: Some(sqrt_price),
            liquidity: None,
        }
    }

    /// Build an upsert payload from a liquidity event.
    ///
    /// `kind` is the domain enum; its mapping to the projection event kind
    /// goes through the [`From<LiquidityEventKind> for LastEventKind`] impl
    /// defined above so add/remove sourcing stays in one place.
    pub fn from_liquidity(
        pool_address: impl Into<String>,
        protocol: impl Into<String>,
        event_at: DateTime<Utc>,
        signature: impl Into<String>,
        kind: LiquidityEventKind,
        reserve_a: u64,
        reserve_b: u64,
        liquidity: u128,
    ) -> Self {
        Self {
            pool_address: pool_address.into(),
            protocol: protocol.into(),
            event_at,
            event_kind: kind.into(),
            signature: signature.into(),
            reserve_a,
            reserve_b,
            sqrt_price: None,
            liquidity: Some(liquidity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_event_kind_roundtrip() {
        for kind in [
            LastEventKind::Swap,
            LastEventKind::LiquidityAdd,
            LastEventKind::LiquidityRemove,
        ] {
            assert_eq!(LastEventKind::from_wire(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn last_event_kind_rejects_unknown() {
        assert_eq!(LastEventKind::from_wire("unknown"), None);
        assert_eq!(LastEventKind::from_wire(""), None);
    }

    #[test]
    fn last_event_kind_from_liquidity_event_kind() {
        assert_eq!(
            LastEventKind::from(LiquidityEventKind::Add),
            LastEventKind::LiquidityAdd
        );
        assert_eq!(
            LastEventKind::from(LiquidityEventKind::Remove),
            LastEventKind::LiquidityRemove
        );
    }

    #[test]
    fn from_swap_marks_kind_as_swap_and_sets_only_sqrt_price() {
        let now = Utc::now();
        let upsert =
            PoolCurrentStateUpsert::from_swap("pool", "damm_v2", now, "sig", 100, 200, 9_999);
        assert_eq!(upsert.event_kind, LastEventKind::Swap);
        assert_eq!(upsert.sqrt_price, Some(9_999));
        assert_eq!(upsert.liquidity, None);
    }

    #[test]
    fn from_liquidity_maps_kind_through_domain_enum() {
        let now = Utc::now();
        let add = PoolCurrentStateUpsert::from_liquidity(
            "pool",
            "damm_v2",
            now,
            "sig",
            LiquidityEventKind::Add,
            100,
            200,
            42,
        );
        let remove = PoolCurrentStateUpsert::from_liquidity(
            "pool",
            "damm_v2",
            now,
            "sig",
            LiquidityEventKind::Remove,
            100,
            200,
            42,
        );
        assert_eq!(add.event_kind, LastEventKind::LiquidityAdd);
        assert_eq!(remove.event_kind, LastEventKind::LiquidityRemove);
        assert_eq!(add.sqrt_price, None);
        assert_eq!(add.liquidity, Some(42));
    }
}
