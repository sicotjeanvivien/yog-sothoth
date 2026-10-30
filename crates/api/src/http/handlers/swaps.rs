//! The swaps, read live, and their measures: their totals, their series, by token — for every
//! protocol, one protocol, or one pool.

use crate::application::TokenActivityRead;
use crate::http::cursor::EventCollection;
use crate::http::{
    AppState,
    dto::{
        PageResponse, PoolSwapBucketResponse, ProtocolSwapActivityBucketResponse,
        ProtocolSwapActivityResponse, SeriesResponse, SwapActivityResponse, SwapEventResponse,
        SwapTotalsResponse, TokenActivityPageResponse,
        request::{
            EventsParams, ListEventsRequest, ListTokenActivityRequest, MeasureActivityParams,
            MeasureWindowParams, SeriesParams, TokenActivityParams, parse_pool, parse_protocol,
        },
    },
    error::ApiError,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{EventScope, Window};

#[utoipa::path(
    get,
    path = "/api/swaps/totals",
    params(MeasureActivityParams),
    responses((
        status = 200,
        description = "Each protocol's swaps over the window.",
        body = SwapTotalsResponse<ProtocolSwapActivityResponse>
    ))
)]
pub(crate) async fn get_swap_totals(
    State(state): State<AppState>,
    Query(params): Query<MeasureActivityParams>,
) -> Result<Json<SwapTotalsResponse<ProtocolSwapActivityResponse>>, ApiError> {
    get_totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/totals",
    params(MeasureActivityParams),
    responses((
        status = 200,
        description = "The protocol's swaps over the window.",
        body = SwapTotalsResponse<ProtocolSwapActivityResponse>
    ))
)]
pub(crate) async fn get_protocol_swap_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<MeasureActivityParams>,
) -> Result<Json<SwapTotalsResponse<ProtocolSwapActivityResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_totals(&state, params, Some(protocol)).await
}

async fn get_totals(
    state: &AppState,
    params: MeasureActivityParams,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<SwapTotalsResponse<ProtocolSwapActivityResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .get_swap_totals(window, params.offset.map(Into::into), protocol)
        .await?;
    Ok(Json(SwapTotalsResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "Each protocol's swaps by `interval`: 5 minutes over the last hour, an hour or 24 hours ending at `asOf` over the other windows.",
        body = SeriesResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_swaps_series(
    State(state): State<AppState>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    list_series(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "The protocol's swaps by `interval`, under the rules of `/api/swaps/series`.",
        body = SeriesResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_series(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_series(&state, params, Some(protocol)).await
}

async fn list_series(
    state: &AppState,
    params: SeriesParams,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<SeriesResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let (window, interval) = params.parse()?;
    let measured = state
        .swap_activity_service
        .list_swap_series(window, interval, protocol)
        .await?;
    Ok(Json(SeriesResponse::new(window, interval, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/totals",
    params(MeasureWindowParams),
    responses((
        status = 200,
        description = "The pool's swaps over the window, 0 when it did not swap; no row before it was first seen.",
        body = SwapTotalsResponse<SwapActivityResponse>
    ))
)]
pub(crate) async fn get_pool_swap_totals(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<SwapTotalsResponse<SwapActivityResponse>>, ApiError> {
    let pool = super::known_pool(&state, &pool).await?;
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .get_pool_swap_totals(window, pool.pool_address)
        .await?;
    Ok(Json(SwapTotalsResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "The pool's swaps by `interval`, under the rules of `/api/swaps/series`.",
        body = SeriesResponse<PoolSwapBucketResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_series(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<PoolSwapBucketResponse>>, ApiError> {
    let (window, interval) = params.parse()?;
    let pool = super::known_pool(&state, &pool).await?;
    let measured = state
        .swap_activity_service
        .list_pool_swap_series(window, interval, pool.pool_address)
        .await?;
    Ok(Json(SeriesResponse::new(window, interval, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/by-token",
    params(TokenActivityParams),
    responses((
        status = 200,
        description = "A page of tokens, the most traded first, every protocol summed.",
        body = TokenActivityPageResponse
    ))
)]
pub(crate) async fn list_swaps_by_token(
    State(state): State<AppState>,
    Query(params): Query<TokenActivityParams>,
) -> Result<Json<TokenActivityPageResponse>, ApiError> {
    let request = ListTokenActivityRequest::parse(params)?;
    match state
        .swap_activity_service
        .list_token_activity(request.window, request.cursor, request.limit)
        .await?
    {
        TokenActivityRead::Page(measured) => Ok(Json(TokenActivityPageResponse::new(
            request.window,
            measured,
        )?)),
        TokenActivityRead::CursorExpired => Err(ApiError::BadRequest(
            "`cursor` belongs to a ranking rewritten since: start again without it".to_string(),
        )),
    }
}

#[utoipa::path(
    get,
    path = "/api/swaps",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of every protocol's swaps, the most recent first; the window ends at the first page.",
        body = PageResponse<SwapEventResponse>
    ))
)]
pub(crate) async fn list_swaps(
    State(state): State<AppState>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let request =
        ListEventsRequest::parse(params, EventCollection::Swaps, EventScope::All, Window::Day)?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the protocol's swaps, the most recent first; the window ends at the first page.",
        body = PageResponse<SwapEventResponse>
    ))
)]
pub(crate) async fn list_protocol_swaps(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::Swaps,
        EventScope::Protocol(protocol),
        Window::Day,
    )?;
    list_event_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps",
    params(EventsParams),
    responses((
        status = 200,
        description = "A page of the pool's swaps, the most recent first; the window ends at the first page.",
        body = PageResponse<SwapEventResponse>
    ))
)]
pub(crate) async fn list_pool_swaps(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<EventsParams>,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let address = parse_pool(&pool)?;
    let request = ListEventsRequest::parse(
        params,
        EventCollection::Swaps,
        EventScope::Pool(address),
        // ⚠️ Raw swaps are kept 7 days (migration 014): a longer window would read a week as more.
        Window::Week,
    )?;
    super::known_pool(&state, &pool).await?;
    list_event_page(&state, request).await
}

async fn list_event_page(
    state: &AppState,
    request: ListEventsRequest,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let page = state
        .swap_event_service
        .list_events(request.scope, request.window, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}

#[cfg(test)]
#[path = "swaps_tests.rs"]
mod tests;
