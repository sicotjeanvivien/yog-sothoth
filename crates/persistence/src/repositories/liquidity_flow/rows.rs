use crate::repositories::helper::{convert_bigdecimal_to_decimal, convert_string_to_pubkey};
use bigdecimal::BigDecimal;
use yog_core::{RepositoryError, domain::PoolLiquidityFlow};

/// Row shape for the liquidity-flow query. The two flow sums are `NUMERIC`
/// (→ `BigDecimal`) forced non-null by the `COALESCE(..., 0)` in the query;
/// `tvl_usd` stays `Option` — a pool the TVL view cannot value must reach
/// the detector as "unvaluable", not as zero.
#[derive(sqlx::FromRow)]
pub(super) struct PoolLiquidityFlowRow {
    pub(super) pool_address: String,
    pub(super) added_usd: BigDecimal,
    pub(super) removed_usd: BigDecimal,
    pub(super) tvl_usd: Option<BigDecimal>,
}

impl TryFrom<PoolLiquidityFlowRow> for PoolLiquidityFlow {
    type Error = RepositoryError;

    fn try_from(row: PoolLiquidityFlowRow) -> Result<Self, Self::Error> {
        Ok(PoolLiquidityFlow {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            added_usd: convert_bigdecimal_to_decimal(row.added_usd, "added_usd")?,
            removed_usd: convert_bigdecimal_to_decimal(row.removed_usd, "removed_usd")?,
            tvl_usd: row
                .tvl_usd
                .map(|v| convert_bigdecimal_to_decimal(v, "tvl_usd"))
                .transpose()?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
