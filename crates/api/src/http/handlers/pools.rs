use axum::{
    Json,
    extract::{Path, Query, State},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use yog_core::{
    domain::{LiquidityCursor, Pool, PoolCursor, SwapCursor},
    tools::Cursor,
};

use crate::bootstrap::AppState;
use crate::http::{
    dto::{
        EmbeddedTokenResponse, LiquidityEventResponse, PageResponse, PoolCurrentStateResponse,
        PoolResponse, SwapEventResponse,
    },
    error::ApiError,
};

/// Default page size when the client does not specify `limit`.
const DEFAULT_LIMIT: i64 = 50;

/// Maximum value accepted from the client. The repository clamps to
/// the same upper bound, but rejecting at the parsing layer gives the
/// client a clearer 400 instead of silent truncation.
const MAX_LIMIT: i64 = 200;

// ===========================================================================
// Query parameters
// ===========================================================================

/// Shared query shape for every paginated endpoint in this module.
///
/// A missing `limit` defaults to `DEFAULT_LIMIT`; an out-of-range value
/// is rejected at the handler with a 400.
#[derive(Debug, Deserialize)]
pub(crate) struct PageQuery {
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

// ===========================================================================
// Path parameter parsing
// ===========================================================================

/// Parse a base58 pool address from the path or return a 400.
fn parse_pool_address(raw: &str) -> Result<solana_pubkey::Pubkey, ApiError> {
    solana_pubkey::Pubkey::from_str(raw)
        .map_err(|_| ApiError::BadRequest(format!("invalid pool address: {raw}")))
}

/// Validate the `limit` query param.
fn validate_limit(limit: i64) -> Result<(), ApiError> {
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "`limit` must be between 1 and {MAX_LIMIT}, got {limit}"
        )));
    }
    Ok(())
}

// ===========================================================================
// GET /api/pools  — collection
// ===========================================================================

/// `GET /api/pools[?cursor=...&limit=...]`
///
/// Returns a paginated list of discovered pools, each enriched with
/// its two token sides. The cursor is opaque from the client's
/// perspective.
pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    validate_limit(query.limit)?;

    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_pool_cursor(raw)?),
        _ => None,
    };

    let page = state
        .pool_repository
        .find_paginated(cursor, query.limit)
        .await?;

    // Enrich every pool in the page. Sequential awaits — the
    // per-pool cost is dominated by DB I/O, and at the page size
    // limit (200 max) the total stays well under 1s.
    let mut items: Vec<PoolResponse> = Vec::with_capacity(page.items.len());
    for pool in page.items {
        items.push(enrich_pool(&state, pool).await?);
    }

    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;

    Ok(Json(PageResponse { items, next_cursor }))
}

// ===========================================================================
// GET /api/pools/{address}  — single resource
// ===========================================================================

/// `GET /api/pools/{address}`
///
/// Returns the pool record enriched with its two token sides.
/// 404 if the pool has never been observed.
pub(crate) async fn get_pool(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<PoolResponse>, ApiError> {
    let pool_address = parse_pool_address(&address)?;

    let pool = state
        .pool_repository
        .find_by_address(&pool_address)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("pool not found: {address}")))?;

    Ok(Json(enrich_pool(&state, pool).await?))
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
    // We validate the address syntactically even though the projection
    // is keyed by `String`: an invalid pubkey would just return None,
    // but rejecting at parse time gives the client a 400 instead of a
    // misleading 404.
    let _ = parse_pool_address(&address)?;

    let state_row = state
        .pool_current_state_repository
        .get_by_address(&address)
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

/// `GET /api/pools/{address}/swaps[?cursor=...&limit=...]`
///
/// Paginated feed of swap events for a single pool, ordered
/// `timestamp DESC, signature ASC`.
pub(crate) async fn list_pool_swaps(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<SwapEventResponse>>, ApiError> {
    let pool_address = parse_pool_address(&address)?;
    validate_limit(query.limit)?;

    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_swap_cursor(raw)?),
        _ => None,
    };

    let page = state
        .swap_event_repository
        .find_by_pool_paginated(&pool_address, cursor, query.limit)
        .await?;

    let items: Vec<SwapEventResponse> = page
        .items
        .into_iter()
        .map(SwapEventResponse::from)
        .collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;

    Ok(Json(PageResponse { items, next_cursor }))
}

// ===========================================================================
// GET /api/pools/{address}/liquidity-events
// ===========================================================================

/// `GET /api/pools/{address}/liquidity-events[?cursor=...&limit=...]`
///
/// Paginated feed of liquidity events (add / remove) for a single pool,
/// ordered `timestamp DESC, signature ASC`.
pub(crate) async fn list_pool_liquidity_events(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageResponse<LiquidityEventResponse>>, ApiError> {
    let pool_address = parse_pool_address(&address)?;
    validate_limit(query.limit)?;

    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_liquidity_cursor(raw)?),
        _ => None,
    };

    let page = state
        .liquidity_event_repository
        .find_by_pool_paginated(&pool_address, cursor, query.limit)
        .await?;

    let items: Vec<LiquidityEventResponse> = page
        .items
        .into_iter()
        .map(LiquidityEventResponse::from)
        .collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;

    Ok(Json(PageResponse { items, next_cursor }))
}

// ===========================================================================
// Cursor wire format
// ===========================================================================
//
// Each cursor variant has its own wire shape so the encoded blob is
// self-describing — a SwapCursor can't be mis-decoded as a PoolCursor
// because the JSON structure won't match. The encoded blob is
// base64(url-safe, no-pad) over a JSON object.
//
// Decoding is variant-specific (the handler knows which kind it expects
// for its endpoint); encoding goes through a single `encode_cursor`
// dispatch on the Cursor enum.

#[derive(Debug, Serialize, Deserialize)]
struct PoolCursorWire {
    first_seen_at: String,
    pool_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventCursorWire {
    timestamp: String,
    signature: String,
}

fn encode_cursor_opt(cursor: Option<&Cursor>) -> Result<Option<String>, ApiError> {
    cursor.map(encode_cursor).transpose()
}

fn encode_cursor(cursor: &Cursor) -> Result<String, ApiError> {
    match cursor {
        Cursor::Pool(c) => encode_b64_json(&PoolCursorWire {
            first_seen_at: c.first_seen_at.to_rfc3339(),
            pool_address: c.pool_address.to_string(),
        }),
        Cursor::Swap(c) => encode_b64_json(&EventCursorWire {
            timestamp: c.timestamp.to_rfc3339(),
            signature: c.signature.clone(),
        }),
        Cursor::Liquidity(c) => encode_b64_json(&EventCursorWire {
            timestamp: c.timestamp.to_rfc3339(),
            signature: c.signature.clone(),
        }),
    }
}

fn encode_b64_json<T: Serialize>(value: &T) -> Result<String, ApiError> {
    let json = serde_json::to_vec(value)
        .map_err(|e| ApiError::Internal(format!("failed to encode cursor: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

fn decode_b64_json<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| ApiError::BadRequest("invalid cursor: not valid base64".to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed payload".to_string()))
}

fn parse_rfc3339(raw: &str) -> Result<chrono::DateTime<chrono::Utc>, ApiError> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed timestamp".to_string()))
}

fn decode_pool_cursor(raw: &str) -> Result<PoolCursor, ApiError> {
    let wire: PoolCursorWire = decode_b64_json(raw)?;
    let first_seen_at = parse_rfc3339(&wire.first_seen_at)?;
    let pool_address = solana_pubkey::Pubkey::from_str(&wire.pool_address)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed pool address".to_string()))?;
    Ok(PoolCursor {
        first_seen_at,
        pool_address,
    })
}

fn decode_swap_cursor(raw: &str) -> Result<SwapCursor, ApiError> {
    let wire: EventCursorWire = decode_b64_json(raw)?;
    Ok(SwapCursor {
        timestamp: parse_rfc3339(&wire.timestamp)?,
        signature: wire.signature,
    })
}

fn decode_liquidity_cursor(raw: &str) -> Result<LiquidityCursor, ApiError> {
    let wire: EventCursorWire = decode_b64_json(raw)?;
    Ok(LiquidityCursor {
        timestamp: parse_rfc3339(&wire.timestamp)?,
        signature: wire.signature,
    })
}

// ===========================================================================
// Enrichment helper
// ===========================================================================

/// Compose a `Pool` with its two embedded token sides.
///
/// Fetches metadata and latest price for both mints, then builds the
/// final `PoolResponse`. Missing metadata or price are tolerated —
/// the corresponding fields will be null in the embedded token, but
/// the pool itself is always returned (a fresh pool may exist in
/// `pools` before `yog-context` has enriched its mints).
///
/// Sequential awaits keep the code readable; at single-request
/// latency the 4 indexed lookups are cheap.
async fn enrich_pool(state: &AppState, pool: Pool) -> Result<PoolResponse, ApiError> {
    let token_a_meta = state
        .token_metadata_repository
        .find_by_mint(&pool.token_a_mint)
        .await?;
    let token_a_price = state
        .token_price_repository
        .find_latest_by_mint(&pool.token_a_mint)
        .await?;
    let token_b_meta = state
        .token_metadata_repository
        .find_by_mint(&pool.token_b_mint)
        .await?;
    let token_b_price = state
        .token_price_repository
        .find_latest_by_mint(&pool.token_b_mint)
        .await?;

    let token_a =
        EmbeddedTokenResponse::from_sources(pool.token_a_mint, token_a_meta, token_a_price);
    let token_b =
        EmbeddedTokenResponse::from_sources(pool.token_b_mint, token_b_meta, token_b_price);

    Ok(PoolResponse::new(pool, token_a, token_b))
}
