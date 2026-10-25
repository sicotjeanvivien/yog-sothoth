use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{PoolSwapBucket, ProtocolBucket, ProtocolSwapActivity, SwapActivity},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, parse_protocol,
};

/// The swap columns every read selects, whatever its scope.
pub(super) struct SwapActivityRow {
    pub(super) swaps: i64,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
}

pub(super) struct ProtocolSwapActivityRow {
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

pub(super) struct ProtocolSwapBucketRow {
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

pub(super) struct PoolSwapBucketRow {
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

fn convert_usd(
    value: Option<BigDecimal>,
    field: &str,
) -> Result<Option<rust_decimal::Decimal>, RepositoryError> {
    convert_optional(value, field, convert_bigdecimal_to_decimal)
}

impl TryFrom<SwapActivityRow> for SwapActivity {
    type Error = RepositoryError;

    fn try_from(row: SwapActivityRow) -> Result<Self, Self::Error> {
        Ok(SwapActivity {
            swaps: row.swaps,
            volume_usd: convert_usd(row.volume_usd, "volume_usd")?,
            fees_usd: convert_usd(row.fees_usd, "fees_usd")?,
            lp_fees_usd: convert_usd(row.lp_fees_usd, "lp_fees_usd")?,
            protocol_fees_usd: convert_usd(row.protocol_fees_usd, "protocol_fees_usd")?,
            referral_fees_usd: convert_usd(row.referral_fees_usd, "referral_fees_usd")?,
            pool_hours: row.pool_hours,
            pool_hours_valued: row.pool_hours_valued,
        })
    }
}

impl TryFrom<ProtocolSwapActivityRow> for ProtocolSwapActivity {
    type Error = RepositoryError;

    fn try_from(row: ProtocolSwapActivityRow) -> Result<Self, Self::Error> {
        Ok(ProtocolSwapActivity {
            protocol: parse_protocol(&row.protocol)?,
            pools: row.pools,
            activity: SwapActivityRow {
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

impl TryFrom<ProtocolSwapBucketRow> for ProtocolBucket<SwapActivity> {
    type Error = RepositoryError;

    fn try_from(row: ProtocolSwapBucketRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBucket {
            protocol: parse_protocol(&row.protocol)?,
            from: row.bar_from,
            value: SwapActivityRow {
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

impl TryFrom<PoolSwapBucketRow> for PoolSwapBucket {
    type Error = RepositoryError;

    fn try_from(row: PoolSwapBucketRow) -> Result<Self, Self::Error> {
        Ok(PoolSwapBucket {
            from: row.bar_from,
            activity: SwapActivityRow {
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
