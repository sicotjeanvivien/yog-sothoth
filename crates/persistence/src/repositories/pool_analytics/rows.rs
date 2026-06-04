use crate::repository_utils::{bigdecimal_to_decimal, convert_string_to_pubkey};
use bigdecimal::BigDecimal;
use solana_pubkey::Pubkey;
use yog_core::{RepositoryError, domain::PoolAnalytics};

/// Row shape for the analytics query. Unlike the other repos this
/// `TryFrom` targets a `(Pubkey, PoolAnalytics)` pair rather than a
/// single domain type — the row carries both the map key and value
/// needed by the caller.
#[derive(sqlx::FromRow)]
pub(super) struct PoolAnalyticsRow {
    pub(super) pool_address: String,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) volume_24h_usd: Option<BigDecimal>,
}

impl TryFrom<PoolAnalyticsRow> for (Pubkey, PoolAnalytics) {
    type Error = RepositoryError;

    fn try_from(row: PoolAnalyticsRow) -> Result<Self, Self::Error> {
        let pool_address = convert_string_to_pubkey(row.pool_address, "pool_address")?;
        let analytics = PoolAnalytics {
            tvl_usd: row
                .tvl_usd
                .map(|v| bigdecimal_to_decimal(v, "tvl_usd"))
                .transpose()?,
            volume_24h_usd: row
                .volume_24h_usd
                .map(|v| bigdecimal_to_decimal(v, "volume_24h_usd"))
                .transpose()?,
        };
        Ok((pool_address, analytics))
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
