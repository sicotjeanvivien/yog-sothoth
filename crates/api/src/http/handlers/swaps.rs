//! The swaps' measures: their totals, their series, by token — for every protocol, one
//! protocol, or one pool.

use crate::application::Cut;
use crate::http::{
    AppState,
    dto::{
        PoolSwapBarResponse, ProtocolSwapBarResponse, ProtocolSwapTotalsResponse, SwapSumsResponse,
        TokenActivityPageResponse, WindowedResponse,
        request::{ActivityQuery, TokenActivityQuery, WindowQuery, parse_protocol},
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
    params(ActivityQuery),
    responses((
        status = 200,
        description = "Each protocol's swaps over the window.",
        body = WindowedResponse<ProtocolSwapTotalsResponse>
    ))
)]
pub(crate) async fn get_swap_totals(
    State(state): State<AppState>,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<WindowedResponse<ProtocolSwapTotalsResponse>>, ApiError> {
    swap_totals(&state, query, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/totals",
    params(ActivityQuery),
    responses((
        status = 200,
        description = "The protocol's swaps over the window.",
        body = WindowedResponse<ProtocolSwapTotalsResponse>
    ))
)]
pub(crate) async fn get_protocol_swap_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<WindowedResponse<ProtocolSwapTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    swap_totals(&state, query, Some(protocol)).await
}

async fn swap_totals(
    state: &AppState,
    query: ActivityQuery,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<WindowedResponse<ProtocolSwapTotalsResponse>>, ApiError> {
    let window = query.window.into();
    let measured = state
        .measure_service
        .swap_totals(window, query.offset.map(Into::into), protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/hourly",
    params(WindowQuery),
    responses((
        status = 200,
        description = "Each protocol's swaps, hour by hour.",
        body = WindowedResponse<ProtocolSwapBarResponse>
    ))
)]
pub(crate) async fn get_swaps_hourly(
    State(state): State<AppState>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolSwapBarResponse>>, ApiError> {
    swap_bars(&state, query, Cut::Hourly, None).await
}

#[utoipa::path(
    get,
    path = "/api/swaps/daily",
    params(WindowQuery),
    responses((
        status = 200,
        description = "Each protocol's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolSwapBarResponse>
    ))
)]
pub(crate) async fn get_swaps_daily(
    State(state): State<AppState>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolSwapBarResponse>>, ApiError> {
    swap_bars(&state, query, Cut::Daily, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/hourly",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The protocol's swaps, hour by hour.",
        body = WindowedResponse<ProtocolSwapBarResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolSwapBarResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    swap_bars(&state, query, Cut::Hourly, Some(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/daily",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The protocol's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolSwapBarResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolSwapBarResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    swap_bars(&state, query, Cut::Daily, Some(protocol)).await
}

async fn swap_bars(
    state: &AppState,
    query: WindowQuery,
    cut: Cut,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<WindowedResponse<ProtocolSwapBarResponse>>, ApiError> {
    let window = query.window.into();
    let measured = state
        .measure_service
        .swap_bars(window, cut, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/totals",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The pool's swaps over the window; no row when it did not swap.",
        body = WindowedResponse<SwapSumsResponse>
    ))
)]
pub(crate) async fn get_pool_swap_totals(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<SwapSumsResponse>>, ApiError> {
    let pool = super::known_pool(&state, &pool).await?;
    let window = query.window.into();
    let measured = state
        .measure_service
        .pool_swap_totals(window, pool.pool_address)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/hourly",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The pool's swaps, hour by hour.",
        body = WindowedResponse<PoolSwapBarResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_hourly(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolSwapBarResponse>>, ApiError> {
    pool_swap_bars(&state, &pool, query, Cut::Hourly).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/daily",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The pool's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolSwapBarResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_daily(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolSwapBarResponse>>, ApiError> {
    pool_swap_bars(&state, &pool, query, Cut::Daily).await
}

async fn pool_swap_bars(
    state: &AppState,
    pool: &str,
    query: WindowQuery,
    cut: Cut,
) -> Result<Json<WindowedResponse<PoolSwapBarResponse>>, ApiError> {
    let pool = super::known_pool(state, pool).await?;
    let window = query.window.into();
    let measured = state
        .measure_service
        .pool_swap_bars(window, cut, pool.pool_address)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/by-token",
    params(TokenActivityQuery),
    responses((
        status = 200,
        description = "A page of tokens, the most traded first, every protocol summed.",
        body = TokenActivityPageResponse
    ))
)]
pub(crate) async fn list_swaps_by_token(
    State(state): State<AppState>,
    Query(query): Query<TokenActivityQuery>,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    super::token::token_ranking(&state, query).await
}

#[cfg(test)]
#[path = "swaps_tests.rs"]
mod tests;
