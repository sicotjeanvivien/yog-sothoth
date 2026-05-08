use chrono::{DateTime, Utc};
use serde::Serialize;
use yog_core::domain::Pool;

/// Wire shape of a pool in API responses.
///
/// Independent from the domain `Pool` so the public contract can evolve
/// (rename `pool_address` → `address`, etc.) without breaking internal
/// representations. Pubkeys are formatted as base58, protocol as
/// snake_case (matching its `Serialize` impl).
#[derive(Debug, Serialize)]
pub(crate) struct PoolResponse {
    pub(crate) address: String,
    pub(crate) protocol: String,
    pub(crate) token_a_mint: String,
    pub(crate) token_b_mint: String,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

impl From<Pool> for PoolResponse {
    fn from(p: Pool) -> Self {
        Self {
            address: p.pool_address.to_string(),
            protocol: p.protocol.to_string(),
            token_a_mint: p.token_a_mint.to_string(),
            token_b_mint: p.token_b_mint.to_string(),
            first_seen_at: p.first_seen_at,
            last_seen_at: p.last_seen_at,
        }
    }
}
