//! `GET /api/tokens/{mint}` — token identity and latest price; and
//! `GET /api/tokens/activity`, what each token was traded for over a window.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::application::TokenActivityRead;
use crate::http::{
    AppState,
    dto::{
        TokenActivityPageResponse, TokenResponse,
        request::{GetTokenRequest, ListTokenActivityRequest, TokenActivityQuery},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/tokens/activity",
    params(TokenActivityQuery),
    responses((
        status = 200,
        description = "A page of tokens, the most traded first.",
        body = TokenActivityPageResponse
    ))
)]
pub(crate) async fn list_token_activity(
    State(state): State<AppState>,
    Query(query): Query<TokenActivityQuery>,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    token_ranking(&state, query).await
}

/// The ranking of the tokens by volume, every protocol summed.
pub(super) async fn token_ranking(
    state: &AppState,
    query: TokenActivityQuery,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    let request = ListTokenActivityRequest::parse(query)?;
    match state
        .activity_service
        .token_activity(request.window, request.cursor, request.limit)
        .await?
    {
        TokenActivityRead::Page(measured) => Ok(Json(TokenActivityPageResponse::new(
            request.window,
            measured,
        )?)),
        TokenActivityRead::CursorExpired => Err(ApiError::BadRequest(
            "`cursor` belongs to a ranking rewritten since: start again without it".to_string(),
        )),
    }
}

#[utoipa::path(
    get,
    path = "/api/tokens/{mint}",
    responses((
        status = 200,
        description = "The token, with its latest price.",
        body = TokenResponse
    ))
)]
pub(crate) async fn get_token(
    State(state): State<AppState>,
    Path(mint): Path<String>,
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
