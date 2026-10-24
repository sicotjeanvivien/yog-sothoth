//! A row of `pool-discoveries/totals`: the pools first seen in the window without their birth
//! observed, and what they hold at the TVL reading of the hour that closes at `at`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolDiscoveries;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolDiscoveriesResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    #[schema(value_type = Count)]
    pools: i64,
    #[schema(value_type = UnsignedDecimal)]
    tvl_usd: Decimal,
}

impl From<PoolDiscoveries> for PoolDiscoveriesResponse {
    fn from(d: PoolDiscoveries) -> Self {
        Self {
            protocol: d.protocol.into(),
            at: d.at,
            pools: d.pools,
            tvl_usd: d.tvl_usd,
        }
    }
}
