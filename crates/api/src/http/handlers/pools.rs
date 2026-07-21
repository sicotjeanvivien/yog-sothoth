use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::bootstrap::AppState;
use crate::http::{
    cursor::encode_cursor_opt,
    dto::{
        FeeTierResponse, LiquidityEventResponse, PageResponse, PoolCurrentStateResponse,
        PoolHistoryBucketResponse, PoolResponse, SwapEventResponse,
        request::{
            GetPoolHistoryRequest, GetPoolLatestStateRequest, GetPoolRequest,
            ListPoolLiquidityRequest, ListPoolSwapsRequest, ListPoolsRequest, ListTopPoolsRequest,
        },
    },
    error::ApiError,
    query::{HistoryQuery, PageQuery, TopPoolsQuery},
};

// ===========================================================================
// GET /api/pools
// ===========================================================================

pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let request = ListPoolsRequest::parse(query)?;
    let page = state.pool_service.list_pools(request.into_query()).await?;

    let items: Vec<PoolResponse> = page.items.into_iter().map(PoolResponse::from).collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;
    let prev_cursor = encode_cursor_opt(page.prev_cursor.as_ref())?;

    Ok(Json(PageResponse {
        items,
        next_cursor,
        prev_cursor,
        is_first: page.is_first,
        is_last: page.is_last,
    }))
}

// ===========================================================================
// GET /api/pools/fee-tiers
// ===========================================================================

/// The most common base-fee tiers (basis points) with their pool counts,
/// ascending by fee — the option list of the pools fee filter. A JSON array
/// of `{ feeBps, poolCount }`; `feeBps` is a decimal string like on the pool
/// responses, `poolCount` a plain number.
pub(crate) async fn list_fee_tiers(
    State(state): State<AppState>,
) -> Result<Json<Vec<FeeTierResponse>>, ApiError> {
    let tiers = state.pool_service.list_fee_tiers().await?;
    let items: Vec<FeeTierResponse> = tiers.into_iter().map(FeeTierResponse::from).collect();
    Ok(Json(items))
}

// ===========================================================================
// GET /api/pools/top
// ===========================================================================

pub(crate) async fn list_top_pools(
    State(state): State<AppState>,
    Query(query): Query<TopPoolsQuery>,
) -> Result<Json<Vec<PoolResponse>>, ApiError> {
    let request = ListTopPoolsRequest::parse(query)?;
    let pools = state
        .pool_service
        .top_pools(request.metric(), request.limit())
        .await?;

    let items: Vec<PoolResponse> = pools.into_iter().map(PoolResponse::from).collect();
    Ok(Json(items))
}

// ===========================================================================
// GET /api/pools/{address}
// ===========================================================================

pub(crate) async fn get_pool(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<PoolResponse>, ApiError> {
    let request = GetPoolRequest::parse(address)?;

    let enriched = state
        .pool_service
        .get_pool(&request.pool_address)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("pool not found: {}", request.pool_address)))?;

    Ok(Json(PoolResponse::from(enriched)))
}

// ===========================================================================
// GET /api/pools/{address}/latest-state
// ===========================================================================

pub(crate) async fn get_pool_latest_state(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<PoolCurrentStateResponse>, ApiError> {
    let request: GetPoolLatestStateRequest = GetPoolLatestStateRequest::parse(address)?;

    let state_row = state
        .pool_service
        .get_latest_state(&request.raw_address)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "no observed state for pool: {} (no swap or liquidity event yet)",
                request.raw_address
            ))
        })?;

    Ok(Json(PoolCurrentStateResponse::from(state_row)))
}

// ===========================================================================
// GET /api/pools/{address}/history
// ===========================================================================

pub(crate) async fn get_pool_history(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<PoolHistoryBucketResponse>>, ApiError> {
    let request = GetPoolHistoryRequest::parse(address, query)?;

    let buckets = state
        .pool_service
        .get_history(&request.pool_address, request.days)
        .await?;

    let items: Vec<PoolHistoryBucketResponse> = buckets
        .into_iter()
        .map(PoolHistoryBucketResponse::from)
        .collect();

    Ok(Json(items))
}

// ===========================================================================
// GET /api/pools/{address}/swaps
// ===========================================================================

pub(crate) async fn list_pool_swaps(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let request = ListPoolSwapsRequest::parse(address, query)?;
    let page = state
        .swap_service
        .list_swaps_for_pool(request.into_params())
        .await?;

    let items: Vec<SwapEventResponse> = page
        .items
        .into_iter()
        .map(SwapEventResponse::from)
        .collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;
    let prev_cursor = encode_cursor_opt(page.prev_cursor.as_ref())?;

    Ok(Json(PageResponse {
        items,
        next_cursor,
        prev_cursor,
        is_first: page.is_first,
        is_last: page.is_last,
    }))
}

// ===========================================================================
// GET /api/pools/{address}/liquidity-events
// ===========================================================================

pub(crate) async fn list_pool_liquidity_events(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<LiquidityEventResponse>>, ApiError> {
    let request = ListPoolLiquidityRequest::parse(address, query)?;
    let page = state
        .liquidity_service
        .list_liquidity_for_pool(request.into_params())
        .await?;

    let items: Vec<LiquidityEventResponse> = page
        .items
        .into_iter()
        .map(LiquidityEventResponse::from)
        .collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;
    let prev_cursor = encode_cursor_opt(page.prev_cursor.as_ref())?;

    Ok(Json(PageResponse {
        items,
        next_cursor,
        prev_cursor,
        is_first: page.is_first,
        is_last: page.is_last,
    }))
}
