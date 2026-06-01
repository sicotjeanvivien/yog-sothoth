use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::bootstrap::AppState;
use crate::http::{
    cursor::encode_cursor_opt,
    dto::{
        LiquidityEventResponse, PageResponse, PoolCurrentStateResponse, PoolResponse,
        SwapEventResponse,
        request::{
            GetPoolLatestStateRequest, GetPoolRequest, ListPoolLiquidityRequest,
            ListPoolSwapsRequest, ListPoolsRequest,
        },
    },
    error::ApiError,
    query::PageQuery,
};

// ===========================================================================
// GET /api/pools
// ===========================================================================

pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    let request = ListPoolsRequest::parse(query)?;
    let page = state.pool_service.list_pools(request.into_params()).await?;

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
    let request = GetPoolLatestStateRequest::parse(address)?;

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
