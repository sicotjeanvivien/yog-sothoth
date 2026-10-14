// ---------------------------------------------------------------------------
// Liquidity event
// ---------------------------------------------------------------------------

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{
    MeteoraDammV2LiquidityEventKind, MeteoraDammV2LiquidityEventValued, Protocol,
};

use crate::http::dto::schema::{U128String, UnsignedDecimal};

/// `GET /api/pools/{address}/liquidity-events` item.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityEventResponse {
    #[schema(min_length = 1)]
    pub(super) pool_address: String,
    #[schema(min_length = 1)]
    pub(super) protocol: String,
    #[schema(min_length = 1)]
    pub(super) signature: String,
    /// Position of this event among its transaction's emissions. A routed
    /// transaction produces several rows sharing `signature` and `timestamp`;
    /// this is what tells them apart — and the only stable key for a list row.
    pub(super) event_index: u16,
    pub(super) timestamp: DateTime<Utc>,

    pub(super) liquidity_event_kind: LiquidityEventKind,
    // u64 quantities emitted as strings (can exceed 2^53); see SwapEventResponse.
    #[schema(value_type = U128String)]
    pub(super) amount_a: String,
    #[schema(value_type = U128String)]
    pub(super) amount_b: String,
    #[schema(value_type = U128String)]
    pub(super) liquidity_delta: String,

    #[schema(value_type = U128String)]
    pub(super) reserve_a_after: String,
    #[schema(value_type = U128String)]
    pub(super) reserve_b_after: String,

    #[schema(min_length = 1)]
    pub(super) position: String,
    #[schema(min_length = 1)]
    pub(super) owner: String,

    /// Trade-time USD value of the event (both legs valued at the price
    /// as-of the event). `null` when not computable (a leg unpriced, or
    /// unresolved mints/decimals). `Decimal` → a string, like the other USD.
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(super) value_usd: Option<Decimal>,
}

impl From<MeteoraDammV2LiquidityEventValued> for LiquidityEventResponse {
    fn from(valued: MeteoraDammV2LiquidityEventValued) -> Self {
        let MeteoraDammV2LiquidityEventValued { event, value_usd } = valued;
        Self {
            pool_address: event.pool_address.to_string(),
            protocol: Protocol::MeteoraDammV2.to_string(),
            signature: event.signature.to_string(),
            event_index: event.event_index,
            timestamp: event.timestamp,
            liquidity_event_kind: event.liquidity_event_kind.into(),
            amount_a: event.amount_a.to_string(),
            amount_b: event.amount_b.to_string(),
            liquidity_delta: event.liquidity_delta.to_string(),
            reserve_a_after: event.reserve_a_after.to_string(),
            reserve_b_after: event.reserve_b_after.to_string(),
            position: event.position.to_string(),
            owner: event.owner.to_string(),
            value_usd,
        }
    }
}

/// Whether liquidity went into the pool or out of it.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LiquidityEventKind {
    Add,
    Remove,
}

impl From<MeteoraDammV2LiquidityEventKind> for LiquidityEventKind {
    fn from(kind: MeteoraDammV2LiquidityEventKind) -> Self {
        match kind {
            MeteoraDammV2LiquidityEventKind::Add => Self::Add,
            MeteoraDammV2LiquidityEventKind::Remove => Self::Remove,
        }
    }
}
