use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use yog_core::domain::{Pool, PoolAnalytics};

use crate::{
    application::{EnrichedPool, EnrichedToken},
    http::dto::EmbeddedTokenResponse,
};

/// Wire shape of a pool in API responses.
///
/// Independent from the domain `Pool` so the public contract can evolve
/// (rename `pool_address` → `address`, etc.) without breaking internal
/// representations. Pubkeys are formatted as base58, protocol as
/// snake_case (matching its `Serialize` impl).
///
/// Analytics (TVL, 24h volume) are denominated in USD. They are
/// `Option` because their computation requires data that may not be
/// available yet (no current state, no priced token, no swap in the
/// window). Serialised as JSON numbers via `rust_decimal`'s exact
/// decimal representation — consistent with the price block in
/// `EmbeddedPriceResponse`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,
    pub(crate) token_a: EmbeddedTokenResponse,
    pub(crate) token_b: EmbeddedTokenResponse,
    /// Base trading fee in basis points (genesis fee tier). `None` until the
    /// pool's `InitializePool` event has been indexed.
    pub(crate) fee_bps: Option<Decimal>,
    pub(crate) tvl_usd: Option<Decimal>,
    pub(crate) volume_24h_usd: Option<Decimal>,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

impl PoolResponse {
    /// Compose the pool with its two enriched token sides and the
    /// derived analytics. The caller (the pool handler) is
    /// responsible for fetching the analytics for the requested
    /// pools in batch — see `enrich_pool` in `handlers/pools.rs`.
    pub(crate) fn new(
        pool: Pool,
        token_a: EmbeddedTokenResponse,
        token_b: EmbeddedTokenResponse,
        analytics: PoolAnalytics,
    ) -> Self {
        Self {
            pool_address: pool.pool_address.to_string(),
            protocol: pool.protocol.to_string(),
            token_a,
            token_b,
            fee_bps: pool.fee_bps,
            tvl_usd: analytics.tvl_usd,
            volume_24h_usd: analytics.volume_24h_usd,
            first_seen_at: pool.first_seen_at,
            last_seen_at: pool.last_seen_at,
        }
    }
}

impl From<EnrichedToken> for EmbeddedTokenResponse {
    fn from(t: EnrichedToken) -> Self {
        EmbeddedTokenResponse::from_sources(t.mint, t.metadata, t.price)
    }
}

impl From<EnrichedPool> for PoolResponse {
    fn from(e: EnrichedPool) -> Self {
        PoolResponse::new(e.pool, e.token_a.into(), e.token_b.into(), e.analytics)
    }
}
