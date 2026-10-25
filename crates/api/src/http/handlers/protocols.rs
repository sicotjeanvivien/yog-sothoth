//! `/api/protocols`: the catalogue, read live, then the measures yog-analytic
//! writes per protocol — activity, its series, the TVL, and the largest liquidity moves.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::http::{
    AppState,
    dto::{
        LatestTvlResponse, LiquidityMoveResponse, ProtocolActivityBarResponse,
        ProtocolActivityResponse, ProtocolCatalogResponse, ProtocolTvlResponse, WindowedResponse,
        request::{ActivityParams, LiquidityMovesParams, ListLiquidityMovesRequest, WindowParams},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols",
    responses((
        status = 200,
        description = "Every protocol of the domain, observed or not.",
        body = Vec<ProtocolCatalogResponse>
    ))
)]
pub(crate) async fn list_protocols(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProtocolCatalogResponse>>, ApiError> {
    let entries = state.protocol_catalog_service.list().await?;
    Ok(Json(entries.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/api/protocols/activity",
    params(ActivityParams),
    responses((
        status = 200,
        description = "Each protocol's activity over the window.",
        body = WindowedResponse<ProtocolActivityResponse>
    ))
)]
pub(crate) async fn get_protocol_activity(
    State(state): State<AppState>,
    Query(params): Query<ActivityParams>,
) -> Result<Json<WindowedResponse<ProtocolActivityResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .activity_service
        .protocol_activity(window, params.offset.map(Into::into))
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/activity/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's activity, hour by hour.",
        body = WindowedResponse<ProtocolActivityBarResponse>
    ))
)]
pub(crate) async fn get_protocol_activity_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolActivityBarResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .activity_service
        .protocol_activity_hourly(window)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/activity/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's activity, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolActivityBarResponse>
    ))
)]
pub(crate) async fn get_protocol_activity_daily(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolActivityBarResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .activity_service
        .protocol_activity_daily(window)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/tvl",
    responses((
        status = 200,
        description = "Each protocol's TVL at the close of the last frozen hour.",
        body = LatestTvlResponse
    ))
)]
pub(crate) async fn get_protocol_tvl(
    State(state): State<AppState>,
) -> Result<Json<LatestTvlResponse>, ApiError> {
    let measured = state.activity_service.protocol_tvl().await?;
    Ok(Json(measured.into()))
}

#[utoipa::path(
    get,
    path = "/api/protocols/tvl/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's TVL at the close of every hour, from the window's start.",
        body = WindowedResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_protocol_tvl_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state.activity_service.protocol_tvl_hourly(window).await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves",
    params(LiquidityMovesParams),
    responses((
        status = 200,
        description = "The protocol's valued liquidity moves over the window, largest first; \
                       none while its liquidity does not cover the window whole.",
        body = WindowedResponse<LiquidityMoveResponse>
    ))
)]
pub(crate) async fn list_liquidity_moves(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<LiquidityMovesParams>,
) -> Result<Json<WindowedResponse<LiquidityMoveResponse>>, ApiError> {
    let request = ListLiquidityMovesRequest::parse(&protocol, params)?;
    let measured = state
        .activity_service
        .liquidity_moves(request.protocol, request.window, request.limit)
        .await?;
    Ok(Json(WindowedResponse::new(request.window, measured)))
}

#[cfg(test)]
#[path = "protocols_tests.rs"]
mod tests;
