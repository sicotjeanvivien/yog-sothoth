//! The pool changes of the pools, read live: every protocol's, one protocol's, one pool's.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{EventScope, Window};

use crate::http::{
    AppState,
    cursor::EventCollection,
    dto::{
        PageResponse, PoolChangeEventResponse,
        request::{EventsParams, ListEventsRequest, parse_pool, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/pool-changes",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of every protocol's pool changes, the most recent first; the window ends at the first page.",
        body = PageResponse<PoolChangeEventResponse>
    ))
)]
pub(crate) async fn list_pool_changes(
    State(state): State<AppState>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PoolChangeEventResponse>>, ApiError> {
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PoolChanges,
        EventScope::All,
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-changes",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the protocol's pool changes, the most recent first; the window ends at the first page.",
        body = PageResponse<PoolChangeEventResponse>
    ))
)]
pub(crate) async fn list_protocol_pool_changes(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PoolChangeEventResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PoolChanges,
        EventScope::Protocol(protocol),
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/pool-changes",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the pool's pool changes, the most recent first; the window ends at the first page.",
        body = PageResponse<PoolChangeEventResponse>
    ))
)]
pub(crate) async fn list_pool_pool_changes(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PoolChangeEventResponse>>, ApiError> {
    let address = parse_pool(&pool)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PoolChanges,
        EventScope::Pool(address),
        Window::Month,
    )?;
    super::known_pool(&state, &pool).await?;
    list_event_page(&state, request).await
}

async fn list_event_page(
    state: &AppState,
    request: ListEventsRequest,
) -> Result<Json<PageResponse<PoolChangeEventResponse>>, ApiError> {
    let page = state
        .pool_change_event_service
        .list_events(request.scope, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}
