//! The swaps of a scope over a period or a bar; the row of a pool's `swaps/totals`, and the
//! fields every other swap row flattens in. `poolHoursValued` of `poolHours` is the coverage.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::SwapSums;

use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapSumsResponse {
    #[schema(value_type = Count)]
    swaps: i64,
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
    pool_hours: i64,
    #[schema(value_type = Count)]
    pool_hours_valued: i64,
}

impl From<SwapSums> for SwapSumsResponse {
    fn from(s: SwapSums) -> Self {
        Self {
            swaps: s.swaps,
            volume_usd: s.volume_usd,
            fees_usd: s.fees_usd,
            lp_fees_usd: s.lp_fees_usd,
            protocol_fees_usd: s.protocol_fees_usd,
            referral_fees_usd: s.referral_fees_usd,
            pool_hours: s.pool_hours,
            pool_hours_valued: s.pool_hours_valued,
        }
    }
}
