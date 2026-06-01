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

use axum::{
    Json,
    extract::{Path, State},
};
use solana_pubkey::Pubkey;
use std::str::FromStr;

use crate::bootstrap::AppState;
use crate::http::{dto::TokenResponse, error::ApiError};

/// `GET /api/tokens/{mint}`
///
/// `mint` is a base58 pubkey; an invalid value returns 400.
pub(crate) async fn get_token(
    State(state): State<AppState>,
    Path(mint): Path<String>,
) -> Result<Json<TokenResponse>, ApiError> {
    let pubkey = Pubkey::from_str(&mint)
        .map_err(|_| ApiError::BadRequest(format!("invalid mint address: {mint}")))?;

    let agg: crate::application::TokenAggregate = state
        .token_service
        .get_token(&pubkey)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("token not found: {mint}")))?;

    Ok(Json(TokenResponse::new(agg.metadata, agg.price)))
}
