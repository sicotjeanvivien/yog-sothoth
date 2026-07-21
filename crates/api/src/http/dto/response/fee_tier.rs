use rust_decimal::Decimal;
use serde::Serialize;
use yog_core::domain::FeeTier;

/// Wire shape of one pools fee-filter option — a base-fee tier and how many
/// pools carry it.
///
/// `fee_bps` is basis points, serialised as a `rust_decimal` string like
/// `feeBps` on the pool responses (precision-safe). `pool_count` is a plain
/// JSON number. The list is the *most common* tiers only, ascending by fee —
/// see [`yog_core::domain::PoolCatalog::list_fee_tiers`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeeTierResponse {
    pub(crate) fee_bps: Decimal,
    pub(crate) pool_count: i64,
}

impl From<FeeTier> for FeeTierResponse {
    fn from(tier: FeeTier) -> Self {
        Self {
            fee_bps: tier.fee_bps,
            pool_count: tier.pool_count,
        }
    }
}
