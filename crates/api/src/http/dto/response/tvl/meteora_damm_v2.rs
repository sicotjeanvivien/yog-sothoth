//! The DAMM v2 block of a TVL reading: its liquidity locked, permanent and vesting.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolTvlMeteoraDammV2;

use crate::http::dto::schema::{Count, UnsignedDecimal};

/// The liquidity locked at the reading, valued at its prices.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolTvlMeteoraDammV2Response {
    /// `permanentUsd + vestingUsd`, over the valued pools.
    #[schema(value_type = UnsignedDecimal)]
    locked_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    permanent_usd: Decimal,
    /// What the vesting schedules still hold at the reading.
    #[schema(value_type = UnsignedDecimal)]
    vesting_usd: Decimal,
    /// Pools holding locked liquidity, valued or not.
    #[schema(value_type = Count)]
    pools_locked: i64,
    /// Those out of the amounts: compounding, range unread, no side priced, or a vesting of
    /// unknown unit.
    #[schema(value_type = Count)]
    pools_locked_unvalued: i64,
    /// The first vesting observed: those locked before it are not counted.
    vesting_covered_since: Option<DateTime<Utc>>,
}

impl From<ProtocolTvlMeteoraDammV2> for ProtocolTvlMeteoraDammV2Response {
    fn from(b: ProtocolTvlMeteoraDammV2) -> Self {
        Self {
            locked_usd: b.locked_usd,
            permanent_usd: b.permanent_usd,
            vesting_usd: b.vesting_usd,
            pools_locked: b.pools_locked,
            pools_locked_unvalued: b.pools_locked_unvalued,
            vesting_covered_since: b.vesting_covered_since,
        }
    }
}
