//! `GET /api/tokens/{mint}` — token identity and latest price.

use axum::{
    Json,
    extract::{Path, State},
};

use crate::bootstrap::AppState;
use crate::http::{
    dto::{TokenResponse, request::GetTokenRequest},
    error::ApiError,
};

pub(crate) async fn get_token(
    State(state): State<AppState>,
    Path(mint): Path<String>,
) -> Result<Json<TokenResponse>, ApiError> {
    let request = GetTokenRequest::parse(mint)?;

    let agg = state
        .token_service
        .get_token(&request.mint)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("token not found: {}", request.mint)))?;

    Ok(Json(TokenResponse::new(agg.metadata, agg.price)))
}
