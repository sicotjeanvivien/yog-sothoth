//! Response DTO for `GET /api/stats`.
//!
//! Protocol-wide aggregate statistics. Kept separate from the domain types
//! like every other `*Response` — the domain model never leaks into the wire
//! shape. Decimals serialize the same way as `PoolResponse` (rust_decimal's
//! native representation).
//!
//! The endpoint is intentionally client-agnostic: it ships raw counters
//! (`poolsPriced` + `poolsObserved`) rather than a formatted "N / M priced"
//! coverage string — the presentation layer derives that. Same for the LP fee
//! share or effective rate: not the API's job here.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::StatsAggregate;
use crate::http::dto::schema::{Count, UnsignedDecimal};

/// Protocol-wide totals. Each USD sum comes with its coverage: `poolsPriced` of
/// `poolsObserved` for the TVL, `swapBucketsPriced24h` of `swapBuckets24h` for
/// volume and fees, since a sum skips what it cannot value.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatsResponse {
    #[schema(value_type = Option<UnsignedDecimal>)]
    total_tvl_usd: Option<Decimal>,
    #[schema(value_type = Count)]
    pools_priced: i64,
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_24h_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    fees_24h_usd: Option<Decimal>,
    #[schema(value_type = Count)]
    swap_buckets_24h: i64,
    #[schema(value_type = Count)]
    swap_buckets_priced_24h: i64,
    #[schema(value_type = Count)]
    pools_observed: i64,
    #[schema(value_type = Count)]
    pools_discovered_24h: i64,
}

impl From<StatsAggregate> for StatsResponse {
    fn from(agg: StatsAggregate) -> Self {
        Self {
            total_tvl_usd: agg.analytics.total_tvl_usd,
            pools_priced: agg.analytics.pools_priced,
            volume_24h_usd: agg.analytics.volume_24h_usd,
            fees_24h_usd: agg.analytics.fees_24h_usd,
            swap_buckets_24h: agg.analytics.swap_buckets_24h,
            swap_buckets_priced_24h: agg.analytics.swap_buckets_priced_24h,
            pools_observed: agg.counts.observed,
            pools_discovered_24h: agg.counts.discovered_24h,
        }
    }
}

#[cfg(test)]
#[path = "tests/stats_tests.rs"]
mod tests;
