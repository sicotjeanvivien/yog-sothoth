use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{ProtocolTvl, ProtocolTvlChange, TvlReading},
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
        })
    }
}
