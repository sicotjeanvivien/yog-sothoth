//! The TVL, a reading at an instant: the last reading, one reading per hour of a window, and its
//! change over a window.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::Protocol;

use crate::http::{
    AppState,
    dto::{
        ProtocolTvlChangeResponse, ProtocolTvlResponse, ReadingResponse, WindowedResponse,
        request::{
            NoQuery, ReadingIntervalParam, ReadingSeriesParams, WindowParams, parse_protocol,
        },
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
    path = "/api/tvl/series",
    params(ReadingSeriesParams),
    responses((
        status = 200,
        description = "Each protocol's TVL reading of every hour measured, from the window's start.",
        body = WindowedResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_tvl_series(
    State(state): State<AppState>,
    Query(params): Query<ReadingSeriesParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    list_readings(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/tvl/series",
    params(ReadingSeriesParams),
    responses((
        status = 200,
        description = "The protocol's TVL reading of every hour measured, from the window's start.",
        body = WindowedResponse<ProtocolTvlResponse>
    ))
)]
pub(crate) async fn get_protocol_tvl_series(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<ReadingSeriesParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_readings(&state, params, Some(protocol)).await
}

async fn list_readings(
    state: &AppState,
    params: ReadingSeriesParams,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<ProtocolTvlResponse>>, ApiError> {
    let ReadingSeriesParams {
        window,
        interval: ReadingIntervalParam::Hour,
    } = params;
    let window = window.into();
    let measured = state
        .protocol_tvl_service
        .list_tvl_hourly(window, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/tvl/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's last TVL reading, the one a window before it, and the \
                       change without the pools discovered in the window; no start but for a \
                       last reading at `asOf`.",
        body = WindowedResponse<ProtocolTvlChangeResponse>
    ))
)]
pub(crate) async fn get_tvl_totals(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlChangeResponse>>, ApiError> {
    get_change(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/tvl/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's last TVL reading, the one a window before it, and the \
                       change without the pools discovered in the window; no start but for a \
                       last reading at `asOf`.",
        body = WindowedResponse<ProtocolTvlChangeResponse>
    ))
)]
pub(crate) async fn get_protocol_tvl_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolTvlChangeResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_change(&state, params, Some(protocol)).await
}

async fn get_change(
    state: &AppState,
    params: WindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<ProtocolTvlChangeResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .protocol_tvl_service
        .get_tvl_change(window, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}
