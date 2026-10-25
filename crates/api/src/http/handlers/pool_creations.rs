//! The pools created: their totals and series, for every protocol or one protocol.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::Protocol;

use crate::application::Cut;
use crate::http::{
    AppState,
    dto::{
        PoolCreationBarResponse, PoolCreationsResponse, WindowedResponse,
        request::{WindowParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/pool-creations/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's pools created over the window.",
        body = WindowedResponse<PoolCreationsResponse>
    ))
)]
pub(crate) async fn get_pool_creation_totals(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolCreationsResponse>>, ApiError> {
    totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created over the window.",
        body = WindowedResponse<PoolCreationsResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creation_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolCreationsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    totals(&state, params, Some(protocol)).await
}

async fn totals(
    state: &AppState,
    params: WindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<PoolCreationsResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .measure_service
        .pool_creation_totals(window, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pool-creations/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's pools created, hour by hour.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_pool_creations_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    bars(&state, params, Cut::Hourly, None).await
}

#[utoipa::path(
    get,
    path = "/api/pool-creations/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's pools created, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_pool_creations_daily(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    bars(&state, params, Cut::Daily, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created, hour by hour.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    bars(&state, params, Cut::Hourly, Some(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    bars(&state, params, Cut::Daily, Some(protocol)).await
}

async fn bars(
    state: &AppState,
    params: WindowParams,
    cut: Cut,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .measure_service
        .pool_creation_bars(window, cut, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}
