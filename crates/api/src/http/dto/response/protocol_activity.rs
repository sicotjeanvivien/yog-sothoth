//! The rows of `/api/protocols/activity`, its series, `/api/protocols/tvl`, and a protocol's
//! largest liquidity moves. Each USD sum comes with its coverage: `poolHoursValued` of
//! `poolHours`, `movesValued` of `moves`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{
    DiscoveredTvl, LatestProtocolTvl, LiquidityFlows, LiquidityMove, LiquidityMoveKind,
    ProtocolActivity, ProtocolActivityBar, ProtocolTvl,
};

use super::protocol::ProtocolResponse;
use super::window::WindowResponse;
use crate::application::Measured;
use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolActivityResponse {
    protocol: ProtocolResponse,
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    lp_fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    protocol_fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    referral_fees_usd: Option<Decimal>,
    #[schema(value_type = Count)]
    swap_count: i64,
    /// Distinct over the window; `null` when no cycle counted them at its last hour.
    #[schema(value_type = Option<Count>)]
    active_pools: Option<i64>,
    /// Pools initialized during the window.
    #[schema(value_type = Count)]
    births: i64,
    #[schema(value_type = Count)]
    pool_hours: i64,
    #[schema(value_type = Count)]
    pool_hours_valued: i64,
    /// `null` when the window starts before the protocol's liquidity was first measured.
    liquidity: Option<LiquidityFlowsResponse>,
}

/// The liquidity added and removed; the sums skip the moves not valued.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityFlowsResponse {
    #[schema(value_type = UnsignedDecimal)]
    added_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    removed_usd: Decimal,
    #[schema(value_type = Count)]
    moves: i64,
    #[schema(value_type = Count)]
    moves_valued: i64,
}

impl From<LiquidityFlows> for LiquidityFlowsResponse {
    fn from(f: LiquidityFlows) -> Self {
        Self {
            added_usd: f.added_usd,
            removed_usd: f.removed_usd,
            moves: f.moves,
            moves_valued: f.moves_valued,
        }
    }
}

impl From<ProtocolActivity> for ProtocolActivityResponse {
    fn from(a: ProtocolActivity) -> Self {
        Self {
            protocol: a.protocol.into(),
            volume_usd: a.volume_usd,
            fees_usd: a.fees_usd,
            lp_fees_usd: a.lp_fees_usd,
            protocol_fees_usd: a.protocol_fees_usd,
            referral_fees_usd: a.referral_fees_usd,
            swap_count: a.swap_count,
            active_pools: a.active_pools,
            births: a.births,
            pool_hours: a.pool_hours,
            pool_hours_valued: a.pool_hours_valued,
            liquidity: a.liquidity.map(Into::into),
        }
    }
}

/// One bar, an hour or 24 hours from `from`. A bar with no row had no swap.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolActivityBarResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    fees_usd: Option<Decimal>,
    #[schema(value_type = Count)]
    swap_count: i64,
    #[schema(value_type = Count)]
    pool_hours: i64,
    #[schema(value_type = Count)]
    pool_hours_valued: i64,
    /// `null` for a bar that starts before the protocol's liquidity was first measured.
    liquidity: Option<LiquidityFlowsResponse>,
}

impl From<ProtocolActivityBar> for ProtocolActivityBarResponse {
    fn from(b: ProtocolActivityBar) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            volume_usd: b.volume_usd,
            fees_usd: b.fees_usd,
            swap_count: b.swap_count,
            pool_hours: b.pool_hours,
            pool_hours_valued: b.pool_hours_valued,
            liquidity: b.liquidity.map(Into::into),
        }
    }
}

/// A protocol's TVL at `at`, the close of the hour it was last measured in.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolTvlResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
    /// Pools with at least one side valued, a side without a price counting 0.
    #[schema(value_type = Count)]
    pools_priced: i64,
    /// Pools with no side valued, out of `tvlUsd`.
    #[schema(value_type = Count)]
    pools_unvalued: i64,
}

impl From<ProtocolTvl> for ProtocolTvlResponse {
    fn from(t: ProtocolTvl) -> Self {
        Self {
            protocol: t.protocol.into(),
            at: t.at,
            tvl_usd: t.tvl_usd,
            pools_priced: t.pools_priced,
            pools_unvalued: t.pools_unvalued,
        }
    }
}

/// The part of `tvlUsd` held by the pools first seen in `period`, their birth unobserved.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscoveredTvlResponse {
    period: WindowResponse,
    #[schema(value_type = UnsignedDecimal)]
    tvl_usd: Decimal,
    #[schema(value_type = Count)]
    pools: i64,
}

impl From<DiscoveredTvl> for DiscoveredTvlResponse {
    fn from(d: DiscoveredTvl) -> Self {
        Self {
            period: d.window.into(),
            tvl_usd: d.tvl_usd,
            pools: d.pools,
        }
    }
}

/// A protocol's last TVL, with its discovered part for each period measured.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LatestProtocolTvlResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
    /// Pools with at least one side valued, a side without a price counting 0.
    #[schema(value_type = Count)]
    pools_priced: i64,
    /// Pools with no side valued, out of `tvlUsd`.
    #[schema(value_type = Count)]
    pools_unvalued: i64,
    /// One per period measured with this TVL; a period missing was not measured.
    discovered: Vec<DiscoveredTvlResponse>,
}

impl From<LatestProtocolTvl> for LatestProtocolTvlResponse {
    fn from(LatestProtocolTvl { tvl, discovered }: LatestProtocolTvl) -> Self {
        Self {
            protocol: tvl.protocol.into(),
            at: tvl.at,
            tvl_usd: tvl.tvl_usd,
            pools_priced: tvl.pools_priced,
            pools_unvalued: tvl.pools_unvalued,
            discovered: discovered.into_iter().map(Into::into).collect(),
        }
    }
}

/// The TVL of every protocol at the close of the last frozen hour.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LatestTvlResponse {
    /// `null` before the first hour is frozen.
    as_of: Option<DateTime<Utc>>,
    items: Vec<LatestProtocolTvlResponse>,
}

impl From<Measured<Vec<LatestProtocolTvl>>> for LatestTvlResponse {
    fn from(measured: Measured<Vec<LatestProtocolTvl>>) -> Self {
        Self {
            as_of: measured.span.map(|s| s.as_of),
            items: measured.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_activity_tests.rs"]
mod tests;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LiquidityMoveKindResponse {
    Add,
    Remove,
}

/// One valued liquidity move, with its pool's token symbols.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityMoveResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    signature: String,
    pool_address: String,
    token_a_symbol: Option<String>,
    token_b_symbol: Option<String>,
    owner: String,
    kind: LiquidityMoveKindResponse,
    #[schema(value_type = UnsignedDecimal)]
    amount_usd: Decimal,
}

impl From<LiquidityMove> for LiquidityMoveResponse {
    fn from(m: LiquidityMove) -> Self {
        Self {
            protocol: m.protocol.into(),
            at: m.at,
            signature: m.signature,
            pool_address: m.pool_address,
            token_a_symbol: m.token_a_symbol,
            token_b_symbol: m.token_b_symbol,
            owner: m.owner,
            kind: match m.kind {
                LiquidityMoveKind::Add => LiquidityMoveKindResponse::Add,
                LiquidityMoveKind::Remove => LiquidityMoveKindResponse::Remove,
            },
            amount_usd: m.amount_usd,
        }
    }
}
