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
        request::{WindowQuery, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/pool-creations/totals",
    params(WindowQuery),
    responses((
        status = 200,
        description = "Each protocol's pools created over the window.",
        body = WindowedResponse<PoolCreationsResponse>
    ))
)]
pub(crate) async fn get_pool_creation_totals(
    State(state): State<AppState>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolCreationsResponse>>, ApiError> {
    totals(&state, query, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/totals",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The protocol's pools created over the window.",
        body = WindowedResponse<PoolCreationsResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creation_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolCreationsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    totals(&state, query, Some(protocol)).await
}

async fn totals(
    state: &AppState,
    query: WindowQuery,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<PoolCreationsResponse>>, ApiError> {
    let window = query.window.into();
    let measured = state
        .measure_service
        .pool_creation_totals(window, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pool-creations/hourly",
    params(WindowQuery),
    responses((
        status = 200,
        description = "Each protocol's pools created, hour by hour.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_pool_creations_hourly(
    State(state): State<AppState>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    bars(&state, query, Cut::Hourly, None).await
}

#[utoipa::path(
    get,
    path = "/api/pool-creations/daily",
    params(WindowQuery),
    responses((
        status = 200,
        description = "Each protocol's pools created, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_pool_creations_daily(
    State(state): State<AppState>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    bars(&state, query, Cut::Daily, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/hourly",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The protocol's pools created, hour by hour.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    bars(&state, query, Cut::Hourly, Some(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/daily",
    params(WindowQuery),
    responses((
        status = 200,
        description = "The protocol's pools created, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolCreationBarResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    bars(&state, query, Cut::Daily, Some(protocol)).await
}

async fn bars(
    state: &AppState,
    query: WindowQuery,
    cut: Cut,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<PoolCreationBarResponse>>, ApiError> {
    let window = query.window.into();
    let measured = state
        .measure_service
        .pool_creation_bars(window, cut, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}
