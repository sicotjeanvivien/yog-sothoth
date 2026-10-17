//! Latest observed on-chain state per pool, upserted by the indexer after each
//! swap or liquidity event it persists.

use solana_pubkey::Pubkey;

use crate::domain::{EventPosition, MeteoraDammV2LiquidityEventKind, Protocol};

/// Kind of the most recent event that touched a pool; mirrors the
/// `last_event_kind` CHECK constraint.
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
}

impl std::fmt::Display for LastEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Here, not beside the event kind: the projection depends on the event, not the reverse.
impl From<MeteoraDammV2LiquidityEventKind> for LastEventKind {
    fn from(kind: MeteoraDammV2LiquidityEventKind) -> Self {
        match kind {
            MeteoraDammV2LiquidityEventKind::Add => LastEventKind::LiquidityAdd,
            MeteoraDammV2LiquidityEventKind::Remove => LastEventKind::LiquidityRemove,
        }
    }
}

/// A state change for [`crate::domain::PoolCurrentStateRepository::upsert`]: only
/// what the event provides; a `None` `sqrt_price` keeps the stored one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCurrentStateUpsert {
    pub pool_address: Pubkey,
    pub protocol: Protocol,

    /// Where the source event sits: the repository orders on it.
    /// ⚠️ Never order on the timestamp alone: many updates share a second.
    pub event_position: EventPosition,
    pub event_kind: LastEventKind,

    pub reserve_a: u64,
    pub reserve_b: u64,

    /// Set only when the upsert originates from a swap event.
    pub sqrt_price: Option<u128>,
}

impl PoolCurrentStateUpsert {
    /// Build an upsert payload from a swap event.
    pub fn from_swap(
        pool_address: Pubkey,
        protocol: Protocol,
        event_position: EventPosition,
        reserve_a: u64,
        reserve_b: u64,
        sqrt_price: u128,
    ) -> Self {
        Self {
            pool_address,
            protocol,
            event_position,
            event_kind: LastEventKind::Swap,
            reserve_a,
            reserve_b,
            sqrt_price: Some(sqrt_price),
        }
    }

    /// Build an upsert payload from a liquidity event.
    /// ⚠️ Not its `liquidity_delta`: one position's movement, not the pool's L.
    pub fn from_liquidity(
        pool_address: Pubkey,
        protocol: Protocol,
        event_position: EventPosition,
        kind: MeteoraDammV2LiquidityEventKind,
        reserve_a: u64,
        reserve_b: u64,
    ) -> Self {
        Self {
            pool_address,
            protocol,
            event_position,
            event_kind: kind.into(),
            reserve_a,
            reserve_b,
            sqrt_price: None,
        }
    }
}

/// What an upsert did, and what it could not know: without `transaction_index`
/// two transactions of one slot are ranked on `event_index`, which only orders
/// the emissions of one transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCurrentStateUpsertOutcome {
    /// `false` when the ordering guard suppressed the write.
    pub applied: bool,

    /// The stored state came from the **same slot**, another signature: the case
    /// the key cannot order. Reported applied or not; a lower bound under concurrency.
    pub same_slot_ambiguity: bool,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
