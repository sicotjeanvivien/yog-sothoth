use crate::repositories::helper::convert_bigdecimal_to_decimal;
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
        let usd = |v: Option<BigDecimal>, field| {
            v.map(|v| convert_bigdecimal_to_decimal(v, field))
                .transpose()
        };
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
