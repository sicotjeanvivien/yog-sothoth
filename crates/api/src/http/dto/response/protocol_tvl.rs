//! A protocol's TVL reading.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolTvl;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

/// A protocol's TVL at `at`, the close of the hour it was last measured in.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolTvlResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
    /// Pools with at least one side valued, a side without a price counting 0.
    #[schema(value_type = Count)]
    pools_priced: i64,
    /// Pools with no side valued, out of `tvlUsd`.
    #[schema(value_type = Count)]
    pools_unvalued: i64,
}

impl From<ProtocolTvl> for ProtocolTvlResponse {
    fn from(t: ProtocolTvl) -> Self {
        Self {
            protocol: t.protocol.into(),
            at: t.at,
            tvl_usd: t.tvl_usd,
            pools_priced: t.pools_priced,
            pools_unvalued: t.pools_unvalued,
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_tvl_tests.rs"]
mod tests;
