//! `GET /api/tokens/{mint}` — token identity and latest price.

use axum::{
    Json,
    extract::{Path, State},
};
use axum_extra::extract::WithRejection;

use crate::http::{
    AppState,
    dto::{TokenResponse, request::GetTokenRequest},
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/tokens/{mint}",
    params(("mint" = String, Path)),
    responses((
        status = 200,
        description = "The token, with its latest price.",
        body = TokenResponse
    ))
)]
pub(crate) async fn get_token(
    State(state): State<AppState>,
    WithRejection(Path(mint), _): WithRejection<Path<String>, ApiError>,
) -> Result<Json<TokenResponse>, ApiError> {
    let request = GetTokenRequest::parse(mint)?;

    let agg: crate::application::TokenAggregate = state
        .token_service
        .get_token(&request.mint)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("token not found: {}", request.mint)))?;

    Ok(Json(TokenResponse::new(agg.metadata, agg.price)))
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
