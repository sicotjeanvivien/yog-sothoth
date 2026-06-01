use crate::http::{
    cursor::{decode_pool_cursor, encode_cursor_opt},
    dto::{PageResponse, PoolResponse},
    error::ApiError,
    query::{
        PageQuery, normalize_search, validate_limit, validate_pagination_query, validate_search,
    },
};
use crate::{
    application::{LiquidityListParams, PoolListParams, SwapListParams},
    http::{
        cursor::{decode_liquidity_cursor, decode_swap_cursor},
        dto::{LiquidityEventResponse, PoolCurrentStateResponse, SwapEventResponse},
    },
};
use crate::{bootstrap::AppState, http::query::validate_cursor_sort_consistency};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use std::str::FromStr;
use yog_core::PoolSort;

// ===========================================================================
// Path parameter parsing
// ===========================================================================

/// Parse a base58 pool address from the path or return a 400.
fn parse_pool_address(raw: &str) -> Result<solana_pubkey::Pubkey, ApiError> {
    solana_pubkey::Pubkey::from_str(raw)
        .map_err(|_| ApiError::BadRequest(format!("invalid pool address: {raw}")))
}

// ===========================================================================
// GET /api/pools  — collection
// ===========================================================================

/// `GET /api/pools[?cursor=...&limit=...]`
///
/// Returns a paginated list of discovered pools, each enriched with
/// its two token sides and its derived analytics (TVL, 24h volume).
/// The cursor is opaque from the client's perspective.
pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    validate_limit(query.limit)?;
    validate_pagination_query(&query)?;
    validate_search(query.q.as_deref())?;

    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_pool_cursor(raw)?),
        _ => None,
    };
    let sort: PoolSort = query.sort.into();

    // Option B payoff: reject a cursor built for a different sort.
    validate_cursor_sort_consistency(cursor.as_ref(), sort)?;

    let params = PoolListParams {
        cursor,
        direction: query.dir.into(),
        position: query.position.map(Into::into),
        sort,
        search: normalize_search(query.q),
        limit: query.limit,
    };
    let page = state.pool_service.list_pools(params).await?;

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
// GET /api/pools/{address}  — single resource
// ===========================================================================

/// `GET /api/pools/{address}`
///
/// Returns the pool record enriched with its two token sides and its
/// derived analytics. 404 if the pool has never been observed.
pub(crate) async fn get_pool(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<PoolResponse>, ApiError> {
    let pool_address = parse_pool_address(&address)?;

    let enriched = state
        .pool_service
        .get_pool(&pool_address)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("pool not found: {address}")))?;

    Ok(Json(PoolResponse::from(enriched)))
}

// ===========================================================================
// GET /api/pools/{address}/latest-state
// ===========================================================================

/// `GET /api/pools/{address}/latest-state`
///
/// Returns the projected current state of the pool (latest reserves,
/// last sqrt_price observed from a swap, last liquidity observed from a
/// liquidity event). 404 if no swap or liquidity event has been
/// observed yet — note that this differs from "pool not found": a pool
/// may exist via Claim* events without ever appearing in the projection.
pub(crate) async fn get_pool_latest_state(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<PoolCurrentStateResponse>, ApiError> {
    let _ = parse_pool_address(&address)?;

    let state_row = state
        .pool_service
        .get_latest_state(&address)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "no observed state for pool: {address} (no swap or liquidity event yet)"
            ))
        })?;

    Ok(Json(PoolCurrentStateResponse::from(state_row)))
}

// ===========================================================================
// GET /api/pools/{address}/swaps
// ===========================================================================

/// `GET /api/pools/{address}/swaps[?cursor=...&dir=...&position=...&limit=...]`
///
/// Paginated feed of swap events for a single pool, ordered
/// `timestamp DESC, signature ASC` (newest first).
///
/// Supports the same bidirectional pagination model as `/api/pools`:
/// `cursor` + `dir` to traverse, `position=first|last` to jump to a
/// list boundary.
pub(crate) async fn list_pool_swaps(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let pool_address = parse_pool_address(&address)?;
    validate_limit(query.limit)?;
    validate_pagination_query(&query)?;

    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_swap_cursor(raw)?),
        _ => None,
    };

    let page = state
        .swap_service
        .list_swaps_for_pool(SwapListParams {
            pool_address,
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            limit: query.limit,
        })
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

/// `GET /api/pools/{address}/liquidity-events[?cursor=...&dir=...&position=...&limit=...]`
///
/// Paginated feed of liquidity events (add / remove) for a single
/// pool, ordered `timestamp DESC, signature ASC` (newest first).
///
/// Supports the same bidirectional pagination model as `/api/pools`.
pub(crate) async fn list_pool_liquidity_events(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<LiquidityEventResponse>>, ApiError> {
    let pool_address = parse_pool_address(&address)?;
    validate_limit(query.limit)?;
    validate_pagination_query(&query)?;

    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_liquidity_cursor(raw)?),
        _ => None,
    };

    let page = state
        .liquidity_service
        .list_liquidity_for_pool(LiquidityListParams {
            pool_address,
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            limit: query.limit,
        })
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
