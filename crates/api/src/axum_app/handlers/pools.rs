use axum::{
    Json,
    extract::{Query, State},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use yog_core::{Cursor, domain::PoolCursor};

use crate::axum_app::error::ApiError;
use crate::bootstrap::AppState;
use crate::interface::http::dto::response::page_response::PageResponse;
use crate::interface::http::dto::response::pool_response::PoolResponse;

/// Default page size when the client does not specify `limit`.
const DEFAULT_LIMIT: i64 = 50;

/// Maximum value accepted from the client. The repository clamps to
/// the same upper bound, but rejecting at the parsing layer gives the
/// client a clearer 400 instead of silent truncation.
const MAX_LIMIT: i64 = 200;

/// Query parameters for `GET /api/pools`.
///
/// `serde(default)` lets axum parse a missing key as the default value.
/// `limit` defaults to `DEFAULT_LIMIT`; range validation happens in the
/// handler so the error message can mention the actual bounds.
#[derive(Debug, Deserialize)]
pub(crate) struct PoolsQuery {
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

/// `GET /api/pools[?cursor=...&limit=...]`
///
/// Returns a paginated list of discovered pools, ordered by
/// `first_seen_at DESC`. The cursor is opaque from the client's
/// perspective — they pass back the `next_cursor` from the previous
/// response without interpreting it.
pub(crate) async fn list_pools(
    State(state): State<AppState>,
    Query(query): Query<PoolsQuery>,
) -> Result<Json<PageResponse<PoolResponse>>, ApiError> {
    // ── Validate limit ──────────────────────────────────────────────────
    if query.limit < 1 || query.limit > MAX_LIMIT {
        return Err(ApiError::BadRequest(format!(
            "`limit` must be between 1 and {MAX_LIMIT}, got {}",
            query.limit
        )));
    }

    // ── Decode cursor (if provided) ─────────────────────────────────────
    let cursor = match query.cursor.as_deref() {
        Some(raw) if !raw.is_empty() => Some(decode_pool_cursor(raw)?),
        _ => None,
    };

    // ── Fetch from the repository ───────────────────────────────────────
    let page = state
        .pool_repository
        .find_paginated(cursor, query.limit)
        .await?;

    // ── Map to the response shape ───────────────────────────────────────
    let items: Vec<PoolResponse> = page.items.into_iter().map(PoolResponse::from).collect();
    let next_cursor = match page.next_cursor.as_ref() {
        Some(c) => Some(encode_cursor(c)?),
        None => None,
    };

    Ok(Json(PageResponse { items, next_cursor }))
}

// ── Cursor wire format ───────────────────────────────────────────────────
//
// Same encoding as the custom stack: base64(url-safe, no-pad) over a
// JSON encoding of the cursor structure. The two stacks must produce
// identical wire output during the transition so cursors issued by one
// can be consumed by the other (manual side-by-side comparison).
//
// Once the custom stack is retired (commit 3), this becomes the single
// source of truth and the duplicate in `interface/http/dto/request/`
// is removed.

#[derive(Debug, Serialize, Deserialize)]
struct PoolCursorWire {
    first_seen_at: String,
    pool_address: String,
}

fn encode_cursor(cursor: &Cursor) -> Result<String, ApiError> {
    match cursor {
        Cursor::Pool(c) => {
            let wire = PoolCursorWire {
                first_seen_at: c.first_seen_at.to_rfc3339(),
                pool_address: c.pool_address.to_string(),
            };
            let json = serde_json::to_vec(&wire)
                .map_err(|e| ApiError::Internal(format!("failed to encode cursor: {e}")))?;
            Ok(URL_SAFE_NO_PAD.encode(json))
        }
    }
}

fn decode_pool_cursor(raw: &str) -> Result<PoolCursor, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| ApiError::BadRequest("invalid cursor: not valid base64".to_string()))?;

    let wire: PoolCursorWire = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed payload".to_string()))?;

    let first_seen_at = chrono::DateTime::parse_from_rfc3339(&wire.first_seen_at)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed timestamp".to_string()))?
        .with_timezone(&chrono::Utc);

    let pool_address = solana_pubkey::Pubkey::from_str(&wire.pool_address)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed pool address".to_string()))?;

    Ok(PoolCursor {
        first_seen_at,
        pool_address,
    })
}
