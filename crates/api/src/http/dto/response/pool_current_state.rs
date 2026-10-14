use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

use super::protocol::Protocol;
use crate::application::PoolCurrentStateView;
use crate::http::dto::schema::{U128String, UnsignedDecimal};

/// `GET /api/pools/{address}/latest-state` response body.
///
/// `reserve_a` / `reserve_b` (u64) and `last_sqrt_price` (u128) are all
/// emitted as JSON strings to preserve their full range across the JS bridge —
/// a JSON-number consumer truncates above 2^53.
///
/// There is no liquidity `L` here, deliberately: the field that claimed to be
/// one published a position's unsigned `liquidity_delta` instead, and was
/// removed in migration 003. Per-movement liquidity lives on
/// `GET /api/pools/{address}/liquidity-events`, under its own name and next to
/// the add/remove kind that gives it a sign.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCurrentStateResponse {
    #[schema(min_length = 1)]
    pub(crate) pool_address: String,
    pub(crate) protocol: Protocol,

    pub(crate) last_event_at: DateTime<Utc>,
    pub(crate) last_event_kind: LastEventKind,
    #[schema(min_length = 1)]
    pub(crate) last_signature: String,

    #[schema(value_type = U128String)]
    pub(crate) reserve_a: String,
    #[schema(value_type = U128String)]
    pub(crate) reserve_b: String,

    /// Q64.64 fixed-point; encoded as a string to keep precision in JS.
    #[schema(value_type = Option<U128String>)]
    pub(crate) last_sqrt_price: Option<String>,

    /// Spot price derived from `last_sqrt_price`: units of token B per 1 token
    /// A, in human (decimal-adjusted) terms. This is the true DAMM v2 price
    /// (concentrated liquidity), **not** the reserve ratio. `None` when there
    /// is no `sqrt_price` yet or the token decimals are unresolved. Serialized
    /// as a string (`rust_decimal` default) to match the other prices.
    #[schema(value_type = Option<UnsignedDecimal>)]
    pub(crate) spot_price_a_in_b: Option<Decimal>,

    pub(crate) last_swap_at: Option<DateTime<Utc>>,

    pub(crate) updated_at: DateTime<Utc>,
}

impl From<PoolCurrentStateView> for PoolCurrentStateResponse {
    fn from(view: PoolCurrentStateView) -> Self {
        let PoolCurrentStateView {
            state,
            spot_price_a_in_b,
        } = view;
        Self {
            pool_address: state.pool_address.to_string(),
            protocol: state.protocol.into(),
            last_event_at: state.last_event_at,
            last_event_kind: state.last_event_kind.into(),
            last_signature: state.last_signature.to_string(),
            reserve_a: state.reserve_a.to_string(),
            reserve_b: state.reserve_b.to_string(),
            last_sqrt_price: state.last_sqrt_price.map(|v| v.to_string()),
            spot_price_a_in_b,
            last_swap_at: state.last_swap_at,
            updated_at: state.updated_at,
        }
    }
}

/// The kind of the event that last changed the pool's state.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LastEventKind {
    Swap,
    LiquidityAdd,
    LiquidityRemove,
}

impl From<domain::LastEventKind> for LastEventKind {
    fn from(kind: domain::LastEventKind) -> Self {
        match kind {
            domain::LastEventKind::Swap => Self::Swap,
            domain::LastEventKind::LiquidityAdd => Self::LiquidityAdd,
            domain::LastEventKind::LiquidityRemove => Self::LiquidityRemove,
        }
    }
}
