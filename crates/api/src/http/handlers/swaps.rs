//! The swaps, read live, and their measures: their totals, their series, by token — for every
//! protocol, one protocol, or one pool.

use crate::application::{BucketPeriod, TokenActivityRead};
use crate::http::cursor::EventCollection;
use crate::http::{
    AppState,
    dto::{
        PageResponse, PoolSwapBucketResponse, ProtocolSwapActivityBucketResponse,
        ProtocolSwapActivityResponse, SwapActivityResponse, SwapEventResponse,
        TokenActivityPageResponse, WindowedResponse,
        request::{
            ActivityParams, EventsParams, ListEventsRequest, ListTokenActivityRequest,
            TokenActivityParams, WindowParams, parse_pool, parse_protocol,
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
    params(ActivityParams),
    responses((
        status = 200,
        description = "Each protocol's swaps over the window.",
        body = WindowedResponse<ProtocolSwapActivityResponse>
    ))
)]
pub(crate) async fn get_swap_totals(
    State(state): State<AppState>,
    Query(params): Query<ActivityParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityResponse>>, ApiError> {
    get_totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/totals",
    params(ActivityParams),
    responses((
        status = 200,
        description = "The protocol's swaps over the window.",
        body = WindowedResponse<ProtocolSwapActivityResponse>
    ))
)]
pub(crate) async fn get_protocol_swap_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<ActivityParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_totals(&state, params, Some(protocol)).await
}

async fn get_totals(
    state: &AppState,
    params: ActivityParams,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .get_swap_totals(window, params.offset.map(Into::into), protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/swaps/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's swaps, hour by hour.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_swaps_hourly(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    list_buckets(&state, params, BucketPeriod::Hour, None).await
}

#[utoipa::path(
    get,
    path = "/api/swaps/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_swaps_daily(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    list_buckets(&state, params, BucketPeriod::Day, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's swaps, hour by hour.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_buckets(&state, params, BucketPeriod::Hour, Some(protocol)).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/swaps/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolSwapActivityBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_swaps_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_buckets(&state, params, BucketPeriod::Day, Some(protocol)).await
}

async fn list_buckets(
    state: &AppState,
    params: WindowParams,
    bucket: BucketPeriod,
    protocol: Option<yog_core::domain::Protocol>,
) -> Result<Json<WindowedResponse<ProtocolSwapActivityBucketResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .list_swap_buckets(window, bucket, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The pool's swaps over the window; no row when it did not swap.",
        body = WindowedResponse<SwapActivityResponse>
    ))
)]
pub(crate) async fn get_pool_swap_totals(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<SwapActivityResponse>>, ApiError> {
    let pool = super::known_pool(&state, &pool).await?;
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .get_pool_swap_totals(window, pool.pool_address)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The pool's swaps, hour by hour.",
        body = WindowedResponse<PoolSwapBucketResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_hourly(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolSwapBucketResponse>>, ApiError> {
    list_pool_buckets(&state, &pool, params, BucketPeriod::Hour).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/swaps/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The pool's swaps, by 24 hours ending at `asOf`.",
        body = WindowedResponse<PoolSwapBucketResponse>
    ))
)]
pub(crate) async fn get_pool_swaps_daily(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolSwapBucketResponse>>, ApiError> {
    list_pool_buckets(&state, &pool, params, BucketPeriod::Day).await
}

async fn list_pool_buckets(
    state: &AppState,
    pool: &str,
    params: WindowParams,
    bucket: BucketPeriod,
) -> Result<Json<WindowedResponse<PoolSwapBucketResponse>>, ApiError> {
    let pool = super::known_pool(state, pool).await?;
    let window = params.window.into();
    let measured = state
        .swap_activity_service
        .list_pool_swap_buckets(window, bucket, pool.pool_address)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
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
