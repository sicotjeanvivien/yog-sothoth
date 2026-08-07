use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, convert_string_to_pubkey,
};
use bigdecimal::BigDecimal;
use yog_core::{RepositoryError, domain::PoolLiquidityFlow};

/// Row shape for the liquidity-flow query. All three `NUMERIC` columns are
/// nullable, for the same reason and with the same meaning: the flow sums are
/// NULL for a window that was not entirely valuable, `tvl_usd` is NULL for a
/// pool the TVL view cannot value. Every one of them must reach the detector
/// as "unvaluable" rather than as a zero it cannot tell from a real one.
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
