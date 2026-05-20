use chrono::{DateTime, Utc};
use serde::Serialize;
use yog_core::domain::Pool;

use crate::http::dto::EmbeddedTokenResponse;

/// Wire shape of a pool in API responses.
///
/// Independent from the domain `Pool` so the public contract can evolve
/// (rename `pool_address` → `address`, etc.) without breaking internal
/// representations. Pubkeys are formatted as base58, protocol as
/// snake_case (matching its `Serialize` impl).
#[derive(Debug, Serialize)]
pub(crate) struct PoolResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,
    pub(crate) token_a: EmbeddedTokenResponse,
    pub(crate) token_b: EmbeddedTokenResponse,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

impl PoolResponse {
    /// Compose the pool with its two enriched token sides.
    ///
    /// The caller (the pool handler) is responsible for fetching the
    /// metadata and price for both mints before calling this — see
    /// `enrich_pool` in `handlers/pools.rs`.
    pub(crate) fn new(
        pool: Pool,
        token_a: EmbeddedTokenResponse,
        token_b: EmbeddedTokenResponse,
    ) -> Self {
        Self {
            pool_address: pool.pool_address.to_string(),
            protocol: pool.protocol.to_string(),
            token_a,
            token_b,
            first_seen_at: pool.first_seen_at,
            last_seen_at: pool.last_seen_at,
        }
    }
}
