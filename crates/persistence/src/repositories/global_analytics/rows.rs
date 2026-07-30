use crate::repositories::helper::{convert_bigdecimal_to_decimal, convert_optional};
use bigdecimal::BigDecimal;
use yog_core::{RepositoryError, domain::GlobalAnalytics};

/// Row shape for the single-row aggregate query. NUMERIC sums map to
/// `BigDecimal`; the priced-pool count is a non-null `BIGINT`.
#[derive(sqlx::FromRow)]
pub(super) struct GlobalAnalyticsRow {
    pub(super) total_tvl_usd: Option<BigDecimal>,
    pub(super) pools_priced: i64,
    pub(super) volume_24h_usd: Option<BigDecimal>,
    pub(super) fees_24h_usd: Option<BigDecimal>,
}

impl TryFrom<GlobalAnalyticsRow> for GlobalAnalytics {
    type Error = RepositoryError;

    fn try_from(row: GlobalAnalyticsRow) -> Result<Self, Self::Error> {
        // Every sum is nullable — an empty window sums to NULL, not to zero.
        let usd = |v, field| convert_optional(v, field, convert_bigdecimal_to_decimal);
        Ok(GlobalAnalytics {
            total_tvl_usd: usd(row.total_tvl_usd, "total_tvl_usd")?,
            pools_priced: row.pools_priced,
            volume_24h_usd: usd(row.volume_24h_usd, "volume_24h_usd")?,
            fees_24h_usd: usd(row.fees_24h_usd, "fees_24h_usd")?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
