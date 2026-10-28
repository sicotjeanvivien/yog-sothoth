use solana_pubkey::Pubkey;

use crate::http::{AppState, dto::request::parse_pool, error::ApiError};

pub(super) mod announcements;
pub(super) mod claim_totals;
pub(super) mod health;
pub(super) mod ingestion;
pub(super) mod liquidity_changes;
pub(super) mod liquidity_moves;
pub(super) mod network;
pub(super) mod pool_changes;
pub(super) mod pool_creations;
pub(super) mod pool_discoveries;
pub(super) mod pools;
pub(super) mod position_change_counts;
pub(super) mod position_changes;
pub(super) mod position_fee_claims;
pub(super) mod protocol_fee_claims;
pub(super) mod protocols;
pub(super) mod reward_changes;
pub(super) mod reward_claims;
pub(super) mod rotation_runs;
pub(super) mod signals;
pub(super) mod swaps;
pub(super) mod token;
pub(super) mod tvl;

/// The token of the path, or a 404 when the catalog has not fetched its metadata.
async fn known_token(state: &AppState, mint: Pubkey) -> Result<(), ApiError> {
    state
        .token_service
        .find_metadata(&mint)
        .await?
        .map(|_| ())
        .ok_or_else(|| ApiError::NotFound(format!("token not found: {mint}")))
}

/// The pool of the path, or a 404 when the catalog does not know it.
async fn known_pool(state: &AppState, raw: &str) -> Result<yog_core::domain::Pool, ApiError> {
    let address: Pubkey = parse_pool(raw)?;
    state
        .pool_service
        .find_pool(address)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("pool not found: {address}")))
}
