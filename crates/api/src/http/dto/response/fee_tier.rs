use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::FeeTier;

use crate::http::dto::schema::{Count, UnsignedDecimal};

/// One option of the pools fee filter: a base-fee tier and how many pools carry
/// it. The list holds the most common tiers only, ascending by fee.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeeTierResponse {
    #[schema(value_type = UnsignedDecimal)]
    fee_bps: Decimal,
    #[schema(value_type = Count)]
    pool_count: i64,
}

impl From<FeeTier> for FeeTierResponse {
    fn from(tier: FeeTier) -> Self {
        Self {
            fee_bps: tier.fee_bps,
            pool_count: tier.pool_count,
        }
    }
}
