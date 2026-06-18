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

use crate::application::StatsAggregate;

/// Protocol-wide statistics payload.
///
/// Three USD aggregates plus two pool counters:
///   - `total_tvl_usd` + `pools_priced`: summed current TVL and how many pools
///     it covers (the coverage numerator; denominator is `pools_observed`).
///   - `volume_24h_usd` / `fees_24h_usd`: summed realized volume and trading
///     fee over the last 24h, trade-time valued.
///   - `pools_observed`: every pool ever seen. `pools_discovered_24h`: those
///     first seen in the last 24h (the discovery pulse).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatsResponse {
    total_tvl_usd: Option<Decimal>,
    pools_priced: i64,
    volume_24h_usd: Option<Decimal>,
    fees_24h_usd: Option<Decimal>,
    pools_observed: i64,
    pools_discovered_24h: i64,
}

impl From<StatsAggregate> for StatsResponse {
    fn from(agg: StatsAggregate) -> Self {
        Self {
            total_tvl_usd: agg.analytics.total_tvl_usd,
            pools_priced: agg.analytics.pools_priced,
            volume_24h_usd: agg.analytics.volume_24h_usd,
            fees_24h_usd: agg.analytics.fees_24h_usd,
            pools_observed: agg.counts.observed,
            pools_discovered_24h: agg.counts.discovered_24h,
        }
    }
}
