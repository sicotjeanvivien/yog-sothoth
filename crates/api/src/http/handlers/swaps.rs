//! The swaps' measures: their totals, their series, by token — for every protocol, one
//! protocol, or one pool.

use crate::application::BucketPeriod;
use crate::http::{
    AppState,
    dto::{
        PoolSwapBucketResponse, ProtocolSwapActivityBucketResponse, ProtocolSwapActivityResponse,
        SwapActivityResponse, TokenActivityPageResponse, WindowedResponse,
        request::{ActivityParams, TokenActivityParams, WindowParams, parse_protocol},
    },
    error::ApiError,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};

#[utoipa::path(
    get,
    path = "/api/swaps/totals",
    params(ActivityParams),
    responses((
        status = 200,
        description = "Each protocol's swaps over the window.",
        body = WindowedResponse<ProtocolSwapActivityResponse>
    ))
)]
pub(crate) async fn get_swap_totals(
    State(state): State<AppState>,
    Query(params): Query<ActivityParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityResponse>>, ApiError> {
    get_totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/totals",
    params(ActivityParams),
    responses((
        status = 200,
        description = "The protocol's swaps over the window.",
        body = WindowedResponse<ProtocolSwapActivityResponse>
    ))
)]
pub(crate) async fn get_protocol_swap_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<ActivityParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_totals(&state, params, Some(protocol)).await
}

async fn get_totals(
    state: &AppState,
    params: ActivityParams,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .get_swap_totals(window, params.offset.map(Into::into), protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's swaps, hour by hour.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_swaps_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    list_buckets(&state, params, BucketPeriod::Hour, None).await
}

#[utoipa::path(
    get,
    path = "/api/swaps/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_swaps_daily(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    list_buckets(&state, params, BucketPeriod::Day, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's swaps, hour by hour.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_buckets(&state, params, BucketPeriod::Hour, Some(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_buckets(&state, params, BucketPeriod::Day, Some(protocol)).await
}

async fn list_buckets(
    state: &AppState,
    params: WindowParams,
    bucket: BucketPeriod,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .list_swap_buckets(window, bucket, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The pool's swaps over the window; no row when it did not swap.",
        body = WindowedResponse<SwapActivityResponse>
    ))
)]
pub(crate) async fn get_pool_swap_totals(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<SwapActivityResponse>>, ApiError> {
    let pool = super::known_pool(&state, &pool).await?;
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .get_pool_swap_totals(window, pool.pool_address)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The pool's swaps, hour by hour.",
        body = WindowedResponse<PoolSwapBucketResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_hourly(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolSwapBucketResponse>>, ApiError> {
    list_pool_buckets(&state, &pool, params, BucketPeriod::Hour).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The pool's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolSwapBucketResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_daily(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolSwapBucketResponse>>, ApiError> {
    list_pool_buckets(&state, &pool, params, BucketPeriod::Day).await
}

async fn list_pool_buckets(
    state: &AppState,
    pool: &str,
    params: WindowParams,
    bucket: BucketPeriod,
) -> Result<Json<WindowedResponse<PoolSwapBucketResponse>>, ApiError> {
    let pool = super::known_pool(state, pool).await?;
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .list_pool_swap_buckets(window, bucket, pool.pool_address)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/by-token",
    params(TokenActivityParams),
    responses((
        status = 200,
        description = "A page of tokens, the most traded first, every protocol summed.",
        body = TokenActivityPageResponse
    ))
)]
pub(crate) async fn list_swaps_by_token(
    State(state): State<AppState>,
    Query(params): Query<TokenActivityParams>,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    super::token::token_ranking(&state, params).await
}

#[cfg(test)]
#[path = "swaps_tests.rs"]
mod tests;
