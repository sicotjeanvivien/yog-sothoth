use solana_pubkey::Pubkey;

use crate::http::{AppState, dto::request::parse_pool, error::ApiError};

pub(super) mod announcements;
pub(super) mod health;
pub(super) mod ingestion;
pub(super) mod liquidity_moves;
pub(super) mod network;
pub(super) mod pool_creations;
pub(super) mod pool_discoveries;
pub(super) mod protocols;
pub(super) mod signals;
pub(super) mod swaps;
pub(super) mod token;
pub(super) mod tvl;

/// The pool of the path, or a 404 when the catalog does not know it.
async fn known_pool(state: &AppState, raw: &str) -> Result<yog_core::domain::Pool, ApiError> {
    let address: Pubkey = parse_pool(raw)?;
    state
        .measure_service
        .pool(address)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("pool not found: {address}")))
}
