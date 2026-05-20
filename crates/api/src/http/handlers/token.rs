//! `GET /api/tokens/{mint}` — token identity and latest price.
//!
//! Assembles two reads:
//!   1. `token_metadata` for the mint;
//!   2. the most recent row of `token_prices` for the same mint.
//!
//! Returns 404 if neither exists (the mint is unknown to the
//! enrichment daemon). A mint with metadata but no price yields a
//! 200 with `price: null` — perfectly valid for fresh or unpriceable
//! mints.

use std::str::FromStr;

use axum::{
    Json,
    extract::{Path, State},
};
use solana_pubkey::Pubkey;

use crate::bootstrap::AppState;
use crate::http::{dto::TokenResponse, error::ApiError};

/// `GET /api/tokens/{mint}`
///
/// `mint` is a base58 pubkey; an invalid value returns 400.
pub(crate) async fn get_token(
    State(state): State<AppState>,
    Path(mint): Path<String>,
) -> Result<Json<TokenResponse>, ApiError> {
    // Parse the path param at the boundary — invalid pubkey is a 400,
    // not a 404 (a 404 would imply "valid but not found").
    let pubkey = Pubkey::from_str(&mint)
        .map_err(|_| ApiError::BadRequest(format!("invalid mint address: {mint}")))?;

    // The two reads are independent; await them sequentially — at
    // single-request latency the back-to-back DB round-trips are
    // negligible, and serial code is easier to read than join!.
    let metadata = state
        .token_metadata_repository
        .find_by_mint(&pubkey)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("token not found: {mint}")))?;

    // No price yet is fine — it'll be `null` in the response.
    let price = state
        .token_price_repository
        .find_latest_by_mint(&pubkey)
        .await?;

    Ok(Json(TokenResponse::new(metadata, price)))
}
