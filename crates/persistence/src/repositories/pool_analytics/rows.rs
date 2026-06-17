use crate::repositories::helper::{convert_bigdecimal_to_decimal, convert_string_to_pubkey};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use yog_core::{
    RepositoryError,
    domain::{PoolAnalytics, PoolHistoryBucket},
};

/// Row shape for the analytics query. Unlike the other repos this
/// `TryFrom` targets a `(Pubkey, PoolAnalytics)` pair rather than a
/// single domain type — the row carries both the map key and value
/// needed by the caller.
#[derive(sqlx::FromRow)]
pub(super) struct PoolAnalyticsRow {
    pub(super) pool_address: String,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) volume_24h_usd: Option<BigDecimal>,
    pub(super) fees_24h_usd: Option<BigDecimal>,
    pub(super) protocol_fees_24h_usd: Option<BigDecimal>,
}

impl TryFrom<PoolAnalyticsRow> for (Pubkey, PoolAnalytics) {
    type Error = RepositoryError;

    fn try_from(row: PoolAnalyticsRow) -> Result<Self, Self::Error> {
        let pool_address = convert_string_to_pubkey(row.pool_address, "pool_address")?;
        let analytics = PoolAnalytics {
            tvl_usd: row
                .tvl_usd
                .map(|v| convert_bigdecimal_to_decimal(v, "tvl_usd"))
                .transpose()?,
            volume_24h_usd: row
                .volume_24h_usd
                .map(|v| convert_bigdecimal_to_decimal(v, "volume_24h_usd"))
                .transpose()?,
            fees_24h_usd: row
                .fees_24h_usd
                .map(|v| convert_bigdecimal_to_decimal(v, "fees_24h_usd"))
                .transpose()?,
            protocol_fees_24h_usd: row
                .protocol_fees_24h_usd
                .map(|v| convert_bigdecimal_to_decimal(v, "protocol_fees_24h_usd"))
                .transpose()?,
        };
        Ok((pool_address, analytics))
    }
}

/// Row shape for one hourly bucket of the pool history query. USD metrics are
/// `NUMERIC` (→ `BigDecimal`); `swap_count` is `BIGINT`. All but `bucket` are
/// nullable (a bucket may have one source of activity but not another).
#[derive(sqlx::FromRow)]
pub(super) struct PoolHistoryRow {
    pub(super) bucket: DateTime<Utc>,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) liquidity_added_usd: Option<BigDecimal>,
    pub(super) liquidity_removed_usd: Option<BigDecimal>,
    pub(super) fees_claimed_usd: Option<BigDecimal>,
    pub(super) rewards_claimed_usd: Option<BigDecimal>,
    pub(super) swap_count: Option<i64>,
}

impl TryFrom<PoolHistoryRow> for PoolHistoryBucket {
    type Error = RepositoryError;

    fn try_from(row: PoolHistoryRow) -> Result<Self, Self::Error> {
        let usd = |v: Option<BigDecimal>, field| {
            v.map(|v| convert_bigdecimal_to_decimal(v, field))
                .transpose()
        };
        Ok(PoolHistoryBucket {
            bucket: row.bucket,
            volume_usd: usd(row.volume_usd, "volume_usd")?,
            fees_usd: usd(row.fees_usd, "fees_usd")?,
            protocol_fees_usd: usd(row.protocol_fees_usd, "protocol_fees_usd")?,
            liquidity_added_usd: usd(row.liquidity_added_usd, "liquidity_added_usd")?,
            liquidity_removed_usd: usd(row.liquidity_removed_usd, "liquidity_removed_usd")?,
            fees_claimed_usd: usd(row.fees_claimed_usd, "fees_claimed_usd")?,
            rewards_claimed_usd: usd(row.rewards_claimed_usd, "rewards_claimed_usd")?,
            swap_count: row.swap_count,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
