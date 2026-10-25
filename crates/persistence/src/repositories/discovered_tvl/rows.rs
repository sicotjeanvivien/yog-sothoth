use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::ProtocolDiscoveredTvl};

use crate::repositories::helper::{convert_bigdecimal_to_decimal, parse_protocol};

pub(super) struct ProtocolDiscoveredTvlRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) pools: i64,
    pub(super) tvl_usd: BigDecimal,
}

impl TryFrom<ProtocolDiscoveredTvlRow> for ProtocolDiscoveredTvl {
    type Error = RepositoryError;

    fn try_from(row: ProtocolDiscoveredTvlRow) -> Result<Self, Self::Error> {
        Ok(ProtocolDiscoveredTvl {
            protocol: parse_protocol(&row.protocol)?,
            at: row.at,
            pools: row.pools,
            tvl_usd: convert_bigdecimal_to_decimal(row.tvl_usd, "tvl_usd")?,
        })
    }
}
