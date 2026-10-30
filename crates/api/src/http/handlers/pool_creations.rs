//! The pools created: their totals and series, for every protocol or one protocol.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{EventScope, Protocol, Window};

use crate::http::cursor::EventCollection;
use crate::http::{
    AppState,
    dto::{
        PageResponse, PoolCreationEventResponse, PoolCreationTotalsResponse,
        ProtocolPoolCreationsBucketResponse, ProtocolPoolCreationsResponse, SeriesResponse,
        request::{
            EventsParams, ListEventsRequest, MeasureWindowParams, SeriesParams, parse_protocol,
        },
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/pool-creations/totals",
    params(MeasureWindowParams),
    responses((
        status = 200,
        description = "Each protocol's pools created over the window.",
        body = PoolCreationTotalsResponse<ProtocolPoolCreationsResponse>
    ))
)]
pub(crate) async fn get_pool_creation_totals(
    State(state): State<AppState>,
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<PoolCreationTotalsResponse<ProtocolPoolCreationsResponse>>, ApiError> {
    get_totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/totals",
    params(MeasureWindowParams),
    responses((
        status = 200,
        description = "The protocol's pools created over the window.",
        body = PoolCreationTotalsResponse<ProtocolPoolCreationsResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creation_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<PoolCreationTotalsResponse<ProtocolPoolCreationsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_totals(&state, params, Some(protocol)).await
}

async fn get_totals(
    state: &AppState,
    params: MeasureWindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<PoolCreationTotalsResponse<ProtocolPoolCreationsResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .pool_creations_service
        .get_pool_creation_totals(window, protocol)
        .await?;
    Ok(Json(PoolCreationTotalsResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pool-creations/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "Each protocol's pools created by `interval`, under the rules of `/api/swaps/series`.",
        body = SeriesResponse<ProtocolPoolCreationsBucketResponse>
    ))
)]
pub(crate) async fn get_pool_creations_series(
    State(state): State<AppState>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    list_series(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-creations/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "The protocol's pools created by `interval`, under the rules of `/api/swaps/series`.",
        body = SeriesResponse<ProtocolPoolCreationsBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_creations_series(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_series(&state, params, Some(protocol)).await
}

async fn list_series(
    state: &AppState,
    params: SeriesParams,
    protocol: Option<Protocol>,
) -> Result<Json<SeriesResponse<ProtocolPoolCreationsBucketResponse>>, ApiError> {
    let (window, interval) = params.parse()?;
    let measured = state
        .pool_creations_service
        .list_pool_creation_series(window, interval, protocol)
        .await?;
    Ok(Json(SeriesResponse::new(window, interval, measured)))
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
    list_event_page(&state, request, None).await
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
    list_event_page(&state, request, Some(protocol)).await
}

async fn list_event_page(
    state: &AppState,
    request: ListEventsRequest,
    protocol: Option<Protocol>,
) -> Result<Json<PageResponse<PoolCreationEventResponse>>, ApiError> {
    let page = state
        .pool_creation_event_service
        .list_events(protocol, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}
