//! The rows of `/api/protocols/activity`, its series, and `/api/protocols/tvl`.
//! Each USD sum comes with its coverage: `poolHoursValued` of `poolHours`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ProtocolActivity, ProtocolActivityBar, ProtocolTvl};

use super::protocol::ProtocolResponse;
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
        }
    }
}

/// A protocol's TVL at `at`, the close of the hour it was last measured in;
/// `poolsPriced` is its coverage.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolTvlResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
    #[schema(value_type = Count)]
    pools_priced: i64,
}

impl From<ProtocolTvl> for ProtocolTvlResponse {
    fn from(t: ProtocolTvl) -> Self {
        Self {
            protocol: t.protocol.into(),
            at: t.at,
            tvl_usd: t.tvl_usd,
            pools_priced: t.pools_priced,
        }
    }
}

/// The TVL of every protocol at the close of the last frozen hour.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LatestTvlResponse {
    /// `null` before the first hour is frozen.
    as_of: Option<DateTime<Utc>>,
    items: Vec<ProtocolTvlResponse>,
}

impl From<Measured<Vec<ProtocolTvl>>> for LatestTvlResponse {
    fn from(measured: Measured<Vec<ProtocolTvl>>) -> Self {
        Self {
            as_of: measured.span.map(|s| s.as_of),
            items: measured.items.into_iter().map(Into::into).collect(),
        }
    }
}
