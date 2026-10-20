use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{ProtocolActivity, ProtocolActivityBar, ProtocolTvl},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, parse_protocol,
};

pub(super) struct ProtocolActivityRow {
    pub(super) protocol: String,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) swap_count: i64,
    pub(super) active_pools: Option<i64>,
    pub(super) births: i64,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
}

pub(super) struct ProtocolActivityBarRow {
    pub(super) protocol: String,
    pub(super) bar_from: DateTime<Utc>,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) swap_count: i64,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
}

pub(super) struct ProtocolTvlRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) pools_priced: i64,
}

fn usd(value: Option<BigDecimal>, field: &str) -> RepositoryResult<Option<rust_decimal::Decimal>> {
    convert_optional(value, field, convert_bigdecimal_to_decimal)
}

impl TryFrom<ProtocolActivityRow> for ProtocolActivity {
    type Error = RepositoryError;

    fn try_from(row: ProtocolActivityRow) -> Result<Self, Self::Error> {
        Ok(ProtocolActivity {
            protocol: parse_protocol(&row.protocol)?,
            volume_usd: usd(row.volume_usd, "volume_usd")?,
            fees_usd: usd(row.fees_usd, "fees_usd")?,
            lp_fees_usd: usd(row.lp_fees_usd, "lp_fees_usd")?,
            protocol_fees_usd: usd(row.protocol_fees_usd, "protocol_fees_usd")?,
            referral_fees_usd: usd(row.referral_fees_usd, "referral_fees_usd")?,
            swap_count: row.swap_count,
            active_pools: row.active_pools,
            births: row.births,
            pool_hours: row.pool_hours,
            pool_hours_valued: row.pool_hours_valued,
        })
    }
}

impl TryFrom<ProtocolActivityBarRow> for ProtocolActivityBar {
    type Error = RepositoryError;

    fn try_from(row: ProtocolActivityBarRow) -> Result<Self, Self::Error> {
        Ok(ProtocolActivityBar {
            protocol: parse_protocol(&row.protocol)?,
            from: row.bar_from,
            volume_usd: usd(row.volume_usd, "volume_usd")?,
            fees_usd: usd(row.fees_usd, "fees_usd")?,
            swap_count: row.swap_count,
            pool_hours: row.pool_hours,
            pool_hours_valued: row.pool_hours_valued,
        })
    }
}

impl TryFrom<ProtocolTvlRow> for ProtocolTvl {
    type Error = RepositoryError;

    fn try_from(row: ProtocolTvlRow) -> Result<Self, Self::Error> {
        Ok(ProtocolTvl {
            protocol: parse_protocol(&row.protocol)?,
            at: row.at,
            tvl_usd: usd(row.tvl_usd, "tvl_usd")?,
            pools_priced: row.pools_priced,
        })
    }
}
