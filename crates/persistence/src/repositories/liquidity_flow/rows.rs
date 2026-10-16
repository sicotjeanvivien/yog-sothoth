use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, convert_string_to_pubkey,
};
use bigdecimal::BigDecimal;
use yog_core::{RepositoryError, domain::PoolLiquidityFlow};

/// Row shape of the liquidity-flow query. A NULL sum (window not entirely valuable)
/// or `tvl_usd` (pool not valued) must reach the detector as unvaluable, not as zero.
#[derive(sqlx::FromRow)]
pub(super) struct PoolLiquidityFlowRow {
    pub(super) pool_address: String,
    pub(super) added_usd: Option<BigDecimal>,
    pub(super) removed_usd: Option<BigDecimal>,
    pub(super) tvl_usd: Option<BigDecimal>,
}

impl TryFrom<PoolLiquidityFlowRow> for PoolLiquidityFlow {
    type Error = RepositoryError;

    fn try_from(row: PoolLiquidityFlowRow) -> Result<Self, Self::Error> {
        Ok(PoolLiquidityFlow {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            added_usd: convert_optional(row.added_usd, "added_usd", convert_bigdecimal_to_decimal)?,
            removed_usd: convert_optional(
                row.removed_usd,
                "removed_usd",
                convert_bigdecimal_to_decimal,
            )?,
            tvl_usd: convert_optional(row.tvl_usd, "tvl_usd", convert_bigdecimal_to_decimal)?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
