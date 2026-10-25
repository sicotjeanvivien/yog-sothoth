//! The liquidity added to and removed from the pools, read live: every protocol's, one
//! protocol's, one pool's.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{EventScope, Window};

use crate::http::{
    AppState,
    cursor::EventCollection,
    dto::{
        LiquidityChangeEventResponse, PageResponse,
        request::{EventsParams, ListEventsRequest, parse_pool, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/liquidity-changes",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of every protocol's liquidity changes, the most recent first; the window ends at the first page.",
        body = PageResponse<LiquidityChangeEventResponse>
    ))
)]
pub(crate) async fn list_liquidity_changes(
    State(state): State<AppState>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<LiquidityChangeEventResponse>>, ApiError> {
    let request = ListEventsRequest::parse(
        params,
        EventCollection::LiquidityChanges,
        EventScope::All,
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-changes",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the protocol's liquidity changes, the most recent first; the window ends at the first page.",
        body = PageResponse<LiquidityChangeEventResponse>
    ))
)]
pub(crate) async fn list_protocol_liquidity_changes(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<LiquidityChangeEventResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::LiquidityChanges,
        EventScope::Protocol(protocol),
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/liquidity-changes",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the pool's liquidity changes, the most recent first; the window ends at the first page.",
        body = PageResponse<LiquidityChangeEventResponse>
    ))
)]
pub(crate) async fn list_pool_liquidity_changes(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<LiquidityChangeEventResponse>>, ApiError> {
    let address = parse_pool(&pool)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::LiquidityChanges,
        EventScope::Pool(address),
        Window::Month,
    )?;
    super::known_pool(&state, &pool).await?;
    list_event_page(&state, request).await
}

async fn list_event_page(
    state: &AppState,
    request: ListEventsRequest,
) -> Result<Json<PageResponse<LiquidityChangeEventResponse>>, ApiError> {
    let page = state
        .liquidity_change_event_service
        .list_events(request.scope, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}
