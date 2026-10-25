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
        PageResponse, TokenActivityPageResponse, TokenPriceResponse, TokenResponse,
        request::{
            GetTokenRequest, ListTokenActivityRequest, ListTokenPricesRequest, ListTokensRequest,
            TokenActivityParams, TokenPricesParams, TokensParams,
        },
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/tokens/activity",
    params(TokenActivityParams),
    responses((
        status = 200,
        description = "A page of tokens, the most traded first.",
        body = TokenActivityPageResponse
    ))
)]
pub(crate) async fn list_token_activity(
    State(state): State<AppState>,
    Query(params): Query<TokenActivityParams>,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    token_ranking(&state, params).await
}

/// The ranking of the tokens by volume, every protocol summed.
pub(super) async fn token_ranking(
    state: &AppState,
    params: TokenActivityParams,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    let request = ListTokenActivityRequest::parse(params)?;
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
