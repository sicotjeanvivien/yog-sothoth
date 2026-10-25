//! The position fee claims of the pools, read live: every protocol's, one protocol's, one pool's.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{EventScope, Window};

use crate::http::{
    AppState,
    cursor::EventCollection,
    dto::{
        PageResponse, PositionFeeClaimEventResponse,
        request::{EventsParams, ListEventsRequest, parse_pool, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/position-fee-claims",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of every protocol's position fee claims, the most recent first; the window ends at the first page.",
        body = PageResponse<PositionFeeClaimEventResponse>
    ))
)]
pub(crate) async fn list_position_fee_claims(
    State(state): State<AppState>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PositionFeeClaimEventResponse>>, ApiError> {
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PositionFeeClaims,
        EventScope::All,
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/position-fee-claims",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the protocol's position fee claims, the most recent first; the window ends at the first page.",
        body = PageResponse<PositionFeeClaimEventResponse>
    ))
)]
pub(crate) async fn list_protocol_position_fee_claims(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PositionFeeClaimEventResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PositionFeeClaims,
        EventScope::Protocol(protocol),
        Window::Month,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/position-fee-claims",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the pool's position fee claims, the most recent first; the window ends at the first page.",
        body = PageResponse<PositionFeeClaimEventResponse>
    ))
)]
pub(crate) async fn list_pool_position_fee_claims(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<PositionFeeClaimEventResponse>>, ApiError> {
    let address = parse_pool(&pool)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::PositionFeeClaims,
        EventScope::Pool(address),
        Window::Month,
    )?;
    super::known_pool(&state, &pool).await?;
    list_event_page(&state, request).await
}

async fn list_event_page(
    state: &AppState,
    request: ListEventsRequest,
) -> Result<Json<PageResponse<PositionFeeClaimEventResponse>>, ApiError> {
    let page = state
        .position_fee_claim_event_service
        .list_events(request.scope, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}
