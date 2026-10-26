use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::ProtocolTvl};

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
