use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{
        ByProtocol, LiquidityMoveSums, PoolCreationCount, PoolDiscoveries, PoolSwapBar,
        ProtocolBar, ProtocolSwapTotals, SwapSums,
    },
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, parse_protocol,
};

/// The swap columns every read selects, whatever its scope.
pub(super) struct SwapSumsRow {
    pub(super) swaps: i64,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
}

pub(super) struct ProtocolSwapTotalsRow {
    pub(super) protocol: String,
    pub(super) swaps: i64,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
    pub(super) pools: Option<i64>,
}

pub(super) struct ProtocolSwapBarRow {
    pub(super) protocol: String,
    pub(super) bar_from: DateTime<Utc>,
    pub(super) swaps: i64,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
}

pub(super) struct PoolSwapBarRow {
    pub(super) bar_from: DateTime<Utc>,
    pub(super) swaps: i64,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
}

pub(super) struct PoolCreationRow {
    pub(super) protocol: String,
    pub(super) bar_from: Option<DateTime<Utc>>,
    pub(super) pools: i64,
}

pub(super) struct LiquidityMoveSumsRow {
    pub(super) protocol: String,
    pub(super) bar_from: Option<DateTime<Utc>>,
    pub(super) created_usd: BigDecimal,
    pub(super) added_usd: BigDecimal,
    pub(super) removed_usd: BigDecimal,
    pub(super) moves: i64,
    pub(super) moves_valued: i64,
}

pub(super) struct PoolDiscoveriesRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) pools: i64,
    pub(super) tvl_usd: BigDecimal,
}

fn usd(
    value: Option<BigDecimal>,
    field: &str,
) -> Result<Option<rust_decimal::Decimal>, RepositoryError> {
    convert_optional(value, field, convert_bigdecimal_to_decimal)
}

impl TryFrom<SwapSumsRow> for SwapSums {
    type Error = RepositoryError;

    fn try_from(row: SwapSumsRow) -> Result<Self, Self::Error> {
        Ok(SwapSums {
            swaps: row.swaps,
            volume_usd: usd(row.volume_usd, "volume_usd")?,
            fees_usd: usd(row.fees_usd, "fees_usd")?,
            lp_fees_usd: usd(row.lp_fees_usd, "lp_fees_usd")?,
            protocol_fees_usd: usd(row.protocol_fees_usd, "protocol_fees_usd")?,
            referral_fees_usd: usd(row.referral_fees_usd, "referral_fees_usd")?,
            pool_hours: row.pool_hours,
            pool_hours_valued: row.pool_hours_valued,
        })
    }
}

impl TryFrom<ProtocolSwapTotalsRow> for ProtocolSwapTotals {
    type Error = RepositoryError;

    fn try_from(row: ProtocolSwapTotalsRow) -> Result<Self, Self::Error> {
        Ok(ProtocolSwapTotals {
            protocol: parse_protocol(&row.protocol)?,
            pools: row.pools,
            sums: SwapSumsRow {
                swaps: row.swaps,
                volume_usd: row.volume_usd,
                fees_usd: row.fees_usd,
                lp_fees_usd: row.lp_fees_usd,
                protocol_fees_usd: row.protocol_fees_usd,
                referral_fees_usd: row.referral_fees_usd,
                pool_hours: row.pool_hours,
                pool_hours_valued: row.pool_hours_valued,
            }
            .try_into()?,
        })
    }
}

impl TryFrom<ProtocolSwapBarRow> for ProtocolBar<SwapSums> {
    type Error = RepositoryError;

    fn try_from(row: ProtocolSwapBarRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBar {
            protocol: parse_protocol(&row.protocol)?,
            from: row.bar_from,
            value: SwapSumsRow {
                swaps: row.swaps,
                volume_usd: row.volume_usd,
                fees_usd: row.fees_usd,
                lp_fees_usd: row.lp_fees_usd,
                protocol_fees_usd: row.protocol_fees_usd,
                referral_fees_usd: row.referral_fees_usd,
                pool_hours: row.pool_hours,
                pool_hours_valued: row.pool_hours_valued,
            }
            .try_into()?,
        })
    }
}

impl TryFrom<PoolSwapBarRow> for PoolSwapBar {
    type Error = RepositoryError;

    fn try_from(row: PoolSwapBarRow) -> Result<Self, Self::Error> {
        Ok(PoolSwapBar {
            from: row.bar_from,
            sums: SwapSumsRow {
                swaps: row.swaps,
                volume_usd: row.volume_usd,
                fees_usd: row.fees_usd,
                lp_fees_usd: row.lp_fees_usd,
                protocol_fees_usd: row.protocol_fees_usd,
                referral_fees_usd: row.referral_fees_usd,
                pool_hours: row.pool_hours,
                pool_hours_valued: row.pool_hours_valued,
            }
            .try_into()?,
        })
    }
}

impl TryFrom<PoolCreationRow> for ByProtocol<PoolCreationCount> {
    type Error = RepositoryError;

    fn try_from(row: PoolCreationRow) -> Result<Self, Self::Error> {
        Ok(ByProtocol {
            protocol: parse_protocol(&row.protocol)?,
            value: PoolCreationCount { pools: row.pools },
        })
    }
}

impl TryFrom<PoolCreationRow> for ProtocolBar<PoolCreationCount> {
    type Error = RepositoryError;

    fn try_from(row: PoolCreationRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBar {
            protocol: parse_protocol(&row.protocol)?,
            from: bar_from(row.bar_from)?,
            value: PoolCreationCount { pools: row.pools },
        })
    }
}

impl TryFrom<LiquidityMoveSumsRow> for LiquidityMoveSums {
    type Error = RepositoryError;

    fn try_from(row: LiquidityMoveSumsRow) -> Result<Self, Self::Error> {
        Ok(LiquidityMoveSums {
            created_usd: convert_bigdecimal_to_decimal(row.created_usd, "created_usd")?,
            added_usd: convert_bigdecimal_to_decimal(row.added_usd, "added_usd")?,
            removed_usd: convert_bigdecimal_to_decimal(row.removed_usd, "removed_usd")?,
            moves: row.moves,
            moves_valued: row.moves_valued,
        })
    }
}

impl TryFrom<LiquidityMoveSumsRow> for ProtocolBar<LiquidityMoveSums> {
    type Error = RepositoryError;

    fn try_from(row: LiquidityMoveSumsRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBar {
            protocol: parse_protocol(&row.protocol)?,
            from: bar_from(row.bar_from)?,
            value: row.try_into()?,
        })
    }
}

impl TryFrom<PoolDiscoveriesRow> for PoolDiscoveries {
    type Error = RepositoryError;

    fn try_from(row: PoolDiscoveriesRow) -> Result<Self, Self::Error> {
        Ok(PoolDiscoveries {
            protocol: parse_protocol(&row.protocol)?,
            at: row.at,
            pools: row.pools,
            tvl_usd: convert_bigdecimal_to_decimal(row.tvl_usd, "tvl_usd")?,
        })
    }
}

fn bar_from(from: Option<DateTime<Utc>>) -> Result<DateTime<Utc>, RepositoryError> {
    from.ok_or_else(|| RepositoryError::Integrity("a bar read without its start".to_owned()))
}
