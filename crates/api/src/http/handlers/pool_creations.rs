//! The pools created: their totals and series, for every protocol or one protocol.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::Protocol;

use crate::application::BucketPeriod;
use crate::http::{
    AppState,
    dto::{
        ProtocolPoolCreationsBucketResponse, ProtocolPoolCreationsResponse, WindowedResponse,
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
        body = WindowedResponse<ProtocolPoolCreationsResponse>
    ))
)]
pub(crate) async fn get_pool_creation_totals(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsResponse>>, ApiError> {
    get_totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created over the window.",
        body = WindowedResponse<ProtocolPoolCreationsResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creation_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_totals(&state, params, Some(protocol)).await
}

async fn get_totals(
    state: &AppState,
    params: WindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .pool_creation_service
        .get_pool_creation_totals(window, protocol)
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
        body = WindowedResponse<ProtocolPoolCreationsBucketResponse>
    ))
)]
pub(crate) async fn get_pool_creations_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    list_buckets(&state, params, BucketPeriod::Hour, None).await
}

#[utoipa::path(
    get,
    path = "/api/pool-creations/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's pools created, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolPoolCreationsBucketResponse>
    ))
)]
pub(crate) async fn get_pool_creations_daily(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    list_buckets(&state, params, BucketPeriod::Day, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created, hour by hour.",
        body = WindowedResponse<ProtocolPoolCreationsBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_buckets(&state, params, BucketPeriod::Hour, Some(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolPoolCreationsBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_buckets(&state, params, BucketPeriod::Day, Some(protocol)).await
}

async fn list_buckets(
    state: &AppState,
    params: WindowParams,
    bucket: BucketPeriod,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .pool_creation_service
        .list_pool_creation_buckets(window, bucket, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}
