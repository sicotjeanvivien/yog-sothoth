//! A row of `pool-discoveries/totals`: the part of a protocol's TVL held by the pools first
//! seen in the window without their birth observed, at the reading of the hour that closes at
//! `at`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolDiscoveredTvl;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolDiscoveredTvlResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    #[schema(value_type = Count)]
    pools: i64,
    #[schema(value_type = UnsignedDecimal)]
    tvl_usd: Decimal,
}

impl From<ProtocolDiscoveredTvl> for ProtocolDiscoveredTvlResponse {
    fn from(d: ProtocolDiscoveredTvl) -> Self {
        Self {
            protocol: d.protocol.into(),
            at: d.at,
            pools: d.pools,
            tvl_usd: d.tvl_usd,
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_discovered_tvl_tests.rs"]
mod tests;
