//! The TVL, a reading at an instant: the last reading, and one reading per hour of a window.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::Protocol;

use crate::http::{
    AppState,
    dto::{
        ProtocolTvlResponse, ReadingResponse, WindowedResponse,
        request::{NoQuery, WindowParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/tvl",
    responses((
        status = 200,
        description = "Each protocol's last TVL reading at or before `asOf`, its instant said.",
        body = ReadingResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_tvl(
    State(state): State<AppState>,
    _query: Query<NoQuery>,
) -> Result<Json<ReadingResponse<ProtocolTvlResponse>>, ApiError> {
    get_latest(&state, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/tvl",
    responses((
        status = 200,
        description = "The protocol's last TVL reading at or before `asOf`, its instant said.",
        body = ReadingResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_protocol_tvl_reading(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    _query: Query<NoQuery>,
) -> Result<Json<ReadingResponse<ProtocolTvlResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_latest(&state, Some(protocol)).await
}

async fn get_latest(
    state: &AppState,
    protocol: Option<Protocol>,
) -> Result<Json<ReadingResponse<ProtocolTvlResponse>>, ApiError> {
    let measured = state.protocol_tvl_service.get_tvl(protocol).await?;
    Ok(Json(ReadingResponse::new(measured)))
}

#[utoipa::path(
    get,
    path = "/api/tvl/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's TVL reading of every hour measured, from the window's start.",
        body = WindowedResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_tvl_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    list_hourly(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/tvl/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's TVL reading of every hour measured, from the window's start.",
        body = WindowedResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_protocol_tvl_readings_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_hourly(&state, params, Some(protocol)).await
}

async fn list_hourly(
    state: &AppState,
    params: WindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .protocol_tvl_service
        .list_tvl_hourly(window, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}
