use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{ProtocolTvl, ProtocolTvlChange, ProtocolTvlMeteoraDammV2, TvlReading},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, parse_protocol,
};

pub(super) struct ProtocolTvlRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) pools_priced: i64,
    pub(super) pools_unvalued: i64,
    pub(super) locked_usd: Option<BigDecimal>,
    pub(super) permanent_usd: Option<BigDecimal>,
    pub(super) vesting_usd: Option<BigDecimal>,
    pub(super) pools_locked: Option<i64>,
    pub(super) pools_locked_unvalued: Option<i64>,
    pub(super) vesting_covered_since: Option<DateTime<Utc>>,
}

/// The locked liquidity's columns of a reading, all `NULL` before it was measured.
struct LockedColumns {
    locked_usd: Option<BigDecimal>,
    permanent_usd: Option<BigDecimal>,
    vesting_usd: Option<BigDecimal>,
    pools_locked: Option<i64>,
    pools_locked_unvalued: Option<i64>,
    vesting_covered_since: Option<DateTime<Utc>>,
}

macro_rules! locked_columns {
    ($row:ident) => {
        LockedColumns {
            locked_usd: $row.locked_usd,
            permanent_usd: $row.permanent_usd,
            vesting_usd: $row.vesting_usd,
            pools_locked: $row.pools_locked,
            pools_locked_unvalued: $row.pools_locked_unvalued,
            vesting_covered_since: $row.vesting_covered_since,
        }
    };
}

impl LockedColumns {
    /// `None` until the reading's locked liquidity was measured: absent, not 0.
    fn into_block(self) -> Result<Option<ProtocolTvlMeteoraDammV2>, RepositoryError> {
        let (Some(locked), Some(permanent), Some(vesting), Some(pools), Some(unvalued)) = (
            self.locked_usd,
            self.permanent_usd,
            self.vesting_usd,
            self.pools_locked,
            self.pools_locked_unvalued,
        ) else {
            return Ok(None);
        };
        Ok(Some(ProtocolTvlMeteoraDammV2 {
            locked_usd: convert_bigdecimal_to_decimal(locked, "locked_usd")?,
            permanent_usd: convert_bigdecimal_to_decimal(permanent, "permanent_usd")?,
            vesting_usd: convert_bigdecimal_to_decimal(vesting, "vesting_usd")?,
            pools_locked: pools,
            pools_locked_unvalued: unvalued,
            vesting_covered_since: self.vesting_covered_since,
        }))
    }
}

impl TryFrom<ProtocolTvlRow> for ProtocolTvl {
    type Error = RepositoryError;

    fn try_from(row: ProtocolTvlRow) -> Result<Self, Self::Error> {
        Ok(ProtocolTvl {
            protocol: parse_protocol(&row.protocol)?,
            at: row.at,
            tvl_usd: convert_optional(row.tvl_usd, "tvl_usd", convert_bigdecimal_to_decimal)?,
            pools_priced: row.pools_priced,
            pools_unvalued: row.pools_unvalued,
            meteora_damm_v2: locked_columns!(row).into_block()?,
        })
    }
}

pub(super) struct ProtocolTvlChangeRow {
    pub(super) protocol: String,
    pub(super) end_at: DateTime<Utc>,
    pub(super) end_tvl_usd: Option<BigDecimal>,
    pub(super) start_at: Option<DateTime<Utc>>,
    pub(super) start_tvl_usd: Option<BigDecimal>,
    pub(super) discovered_tvl_usd: Option<BigDecimal>,
    pub(super) locked_usd: Option<BigDecimal>,
    pub(super) permanent_usd: Option<BigDecimal>,
    pub(super) vesting_usd: Option<BigDecimal>,
    pub(super) pools_locked: Option<i64>,
    pub(super) pools_locked_unvalued: Option<i64>,
    pub(super) vesting_covered_since: Option<DateTime<Utc>>,
}

impl TryFrom<ProtocolTvlChangeRow> for ProtocolTvlChange {
    type Error = RepositoryError;

    fn try_from(row: ProtocolTvlChangeRow) -> Result<Self, Self::Error> {
        let decimal = |value, field| convert_optional(value, field, convert_bigdecimal_to_decimal);
        Ok(ProtocolTvlChange {
            protocol: parse_protocol(&row.protocol)?,
            end: TvlReading {
                at: row.end_at,
                tvl_usd: decimal(row.end_tvl_usd, "end_tvl_usd")?,
            },
            start: row
                .start_at
                .map(|at| {
                    Ok::<_, RepositoryError>(TvlReading {
                        at,
                        tvl_usd: decimal(row.start_tvl_usd, "start_tvl_usd")?,
                    })
                })
                .transpose()?,
            discovered_tvl_usd: decimal(row.discovered_tvl_usd, "discovered_tvl_usd")?,
            meteora_damm_v2: locked_columns!(row).into_block()?,
        })
    }
}
