//! The pools created: their totals and series, for every protocol or one protocol.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{EventScope, Protocol, Window};

use crate::application::BucketPeriod;
use crate::http::cursor::EventCollection;
use crate::http::{
    AppState,
    dto::{
        PageResponse, PoolCreationEventResponse, ProtocolPoolCreationsBucketResponse,
        ProtocolPoolCreationsResponse, WindowedResponse,
        request::{EventsParams, ListEventsRequest, WindowParams, parse_protocol},
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
        .pool_creations_service
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
        .pool_creations_service
        .list_pool_creation_buckets(window, bucket, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pool-creations",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of every protocol's pool creations, the most recent first; the window ends at the first page.",
        body = PageResponse<PoolCreationEventResponse>
    ))
)]
pub(crate) async fn list_pool_creations(
    State(state): State<AppState>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PoolCreationEventResponse>>, ApiError> {
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PoolCreations,
        EventScope::All,
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the protocol's pool creations, the most recent first; the window ends at the first page.",
        body = PageResponse<PoolCreationEventResponse>
    ))
)]
pub(crate) async fn list_protocol_pool_creations(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PoolCreationEventResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PoolCreations,
        EventScope::Protocol(protocol),
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

async fn list_event_page(
    state: &AppState,
    request: ListEventsRequest,
) -> Result<Json<PageResponse<PoolCreationEventResponse>>, ApiError> {
    let page = state
        .pool_creation_event_service
        .list_events(request.scope, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}
