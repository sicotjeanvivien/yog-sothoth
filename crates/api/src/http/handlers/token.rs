//! `GET /api/tokens/{mint}` — token identity and latest price — and the lists of tokens and of a
//! token's prices.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::http::{
    AppState,
    dto::{
        PageResponse, TokenPriceResponse, TokenResponse,
        request::{
            GetTokenRequest, ListTokenPricesRequest, ListTokensRequest, NoQuery, TokenPricesParams,
            TokensParams,
        },
    },
    error::ApiError,
};

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
    _query: Query<NoQuery>,
) -> Result<Json<TokenResponse>, ApiError> {
    let request = GetTokenRequest::parse(mint)?;

    let agg: crate::application::TokenAggregate = state
        .token_service
        .get_token(&request.mint)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("token not found: {}", request.mint)))?;

    Ok(Json(TokenResponse::new(agg.metadata, agg.price)))
}

#[utoipa::path(
    get,
    path = "/api/tokens",
    params(TokensParams),
    responses((
        status = 200,
        description = "A page of tokens, the most recently first fetched first, each with its \
                       latest price.",
        body = PageResponse<TokenResponse>
    ))
)]
pub(crate) async fn list_tokens(
    State(state): State<AppState>,
    Query(params): Query<TokensParams>,
) -> Result<Json<PageResponse<TokenResponse>>, ApiError> {
    let request = ListTokensRequest::parse(params)?;
    let page = state
        .token_service
        .list_tokens(request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}

#[utoipa::path(
    get,
    path = "/api/tokens/{mint}/prices",
    params(TokenPricesParams),
    responses((
        status = 200,
        description = "A page of the token's prices over the window, the most recent first; \
                       the window ends at the first page.",
        body = PageResponse<TokenPriceResponse>
    ))
)]
pub(crate) async fn list_token_prices(
    State(state): State<AppState>,
    Path(mint): Path<String>,
    Query(params): Query<TokenPricesParams>,
) -> Result<Json<PageResponse<TokenPriceResponse>>, ApiError> {
    let request = ListTokenPricesRequest::parse(&mint, params)?;
    super::known_token(&state, request.mint).await?;
    let page = state
        .token_service
        .list_token_prices(&request.mint, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
