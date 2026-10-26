//! The pools, read live: every protocol's, one protocol's, a token's, and one pool whole.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::PoolScope;

use crate::http::{
    AppState,
    dto::{
        PageResponse, PoolDetailResponse, PoolResponse,
        request::{ListPoolsRequest, NoQuery, PoolsParams, parse_mint, parse_pool, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/pools",
    params(PoolsParams),
    responses((
        status = 200,
        description = "A page of every protocol's pools, the most recently first seen first.",
        body = PageResponse<PoolResponse>
    ))
)]
pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(params): Query<PoolsParams>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    list_scope(&state, params, PoolScope::All).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pools",
    params(PoolsParams),
    responses((
        status = 200,
        description = "A page of the protocol's pools, the most recently first seen first.",
        body = PageResponse<PoolResponse>
    ))
)]
pub(crate) async fn list_protocol_pools(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<PoolsParams>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_scope(&state, params, PoolScope::Protocol(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/tokens/{mint}/pools",
    params(PoolsParams),
    responses((
        status = 200,
        description = "A page of the pools the token is either side of, the most recently \
                       first seen first.",
        body = PageResponse<PoolResponse>
    ))
)]
pub(crate) async fn list_token_pools(
    State(state): State<AppState>,
    Path(mint): Path<String>,
    Query(params): Query<PoolsParams>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let mint = parse_mint(&mint)?;
    let request = ListPoolsRequest::parse(params, PoolScope::Token(mint))?;
    super::known_token(&state, mint).await?;
    list_request(&state, request).await
}

async fn list_scope(
    state: &AppState,
    params: PoolsParams,
    scope: PoolScope,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let request = ListPoolsRequest::parse(params, scope)?;
    list_request(state, request).await
}

async fn list_request(
    state: &AppState,
    request: ListPoolsRequest,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let page = state
        .pool_service
        .list_pools(request.scope, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}",
    responses((
        status = 200,
        description = "The pool, its state now, and its protocol's properties.",
        body = PoolDetailResponse
    ))
)]
pub(crate) async fn get_pool(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    _query: Query<NoQuery>,
) -> Result<Json<PoolDetailResponse>, ApiError> {
    let address = parse_pool(&pool)?;
    let detail = state
        .pool_service
        .get_pool(address)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("pool not found: {address}")))?;
    Ok(Json(detail.into()))
}

#[cfg(test)]
#[path = "pools_tests.rs"]
mod tests;
