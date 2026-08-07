use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, convert_string_to_pubkey,
};
use bigdecimal::BigDecimal;
use yog_core::{RepositoryError, domain::PoolSwapFlow};

/// Row shape for the directional-flow query. Both USD sums are `NUMERIC`
/// (→ `BigDecimal`) and **nullable**: the query returns NULL for a window that
/// was not entirely valuable, so "unknown" reaches the detector as itself
/// instead of as a zero it cannot tell from a real one.
#[derive(sqlx::FromRow)]
pub(super) struct PoolSwapFlowRow {
    pub(super) pool_address: String,
    pub(super) volume_a_to_b_usd: Option<BigDecimal>,
    pub(super) volume_b_to_a_usd: Option<BigDecimal>,
}

impl TryFrom<PoolSwapFlowRow> for PoolSwapFlow {
    type Error = RepositoryError;

    fn try_from(row: PoolSwapFlowRow) -> Result<Self, Self::Error> {
        Ok(PoolSwapFlow {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            volume_a_to_b_usd: convert_optional(
                row.volume_a_to_b_usd,
                "volume_a_to_b_usd",
                convert_bigdecimal_to_decimal,
            )?,
            volume_b_to_a_usd: convert_optional(
                row.volume_b_to_a_usd,
                "volume_b_to_a_usd",
                convert_bigdecimal_to_decimal,
            )?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
