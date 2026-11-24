use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::MeteoraDammV2LpFeeSplit;

use crate::http::dto::schema::UnsignedDecimal;

/// What a DAMM v2 row says beyond every protocol's: its LPs' fees claimed and compounded into
/// the pool, which sum to `lpFeesUsd`; each `null` where not measured.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapActivityMeteoraDammV2Response {
    #[schema(value_type = Option<UnsignedDecimal>)]
    claiming_fees_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    compounding_fees_usd: Option<Decimal>,
}

impl From<MeteoraDammV2LpFeeSplit> for SwapActivityMeteoraDammV2Response {
    fn from(split: MeteoraDammV2LpFeeSplit) -> Self {
        Self {
            claiming_fees_usd: split.claiming_fees_usd,
            compounding_fees_usd: split.compounding_fees_usd,
        }
    }
}
