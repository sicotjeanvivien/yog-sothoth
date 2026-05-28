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
use crate::http::error::ApiError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use yog_core::{
    Cursor, PoolSortColumn,
    domain::{LiquidityCursor, PoolCursor, SwapCursor},
};

#[derive(Debug, Serialize, Deserialize)]
struct PoolCursorWire {
    sort_column: String,
    sort_value: String,
    pool_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventCursorWire {
    timestamp: String,
    signature: String,
}

pub(crate) fn encode_cursor_opt(cursor: Option<&Cursor>) -> Result<Option<String>, ApiError> {
    cursor.map(encode_cursor).transpose()
}

pub(crate) fn encode_cursor(cursor: &Cursor) -> Result<String, ApiError> {
    match cursor {
        Cursor::Pool(c) => encode_b64_json(&PoolCursorWire {
            sort_column: sort_column_to_wire(c.sort_column).to_string(),
            sort_value: c.sort_value.to_rfc3339(),
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

pub(crate) fn encode_b64_json<T: Serialize>(value: &T) -> Result<String, ApiError> {
    let json = serde_json::to_vec(value)
        .map_err(|e| ApiError::Internal(format!("failed to encode cursor: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

pub(crate) fn decode_b64_json<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| ApiError::BadRequest("invalid cursor: not valid base64".to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed payload".to_string()))
}

pub(crate) fn parse_rfc3339(raw: &str) -> Result<chrono::DateTime<chrono::Utc>, ApiError> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed timestamp".to_string()))
}

pub(crate) fn decode_pool_cursor(raw: &str) -> Result<PoolCursor, ApiError> {
    let wire: PoolCursorWire = decode_b64_json(raw)?;
    let sort_column = sort_column_from_wire(&wire.sort_column)?;
    let sort_value = parse_rfc3339(&wire.sort_value)?;
    let pool_address = solana_pubkey::Pubkey::from_str(&wire.pool_address)
        .map_err(|_| ApiError::BadRequest("invalid cursor: malformed pool address".to_string()))?;
    Ok(PoolCursor {
        sort_column,
        sort_value,
        pool_address,
    })
}

pub(crate) fn decode_swap_cursor(raw: &str) -> Result<SwapCursor, ApiError> {
    let wire: EventCursorWire = decode_b64_json(raw)?;
    Ok(SwapCursor {
        timestamp: parse_rfc3339(&wire.timestamp)?,
        signature: wire.signature,
    })
}

pub(crate) fn decode_liquidity_cursor(raw: &str) -> Result<LiquidityCursor, ApiError> {
    let wire: EventCursorWire = decode_b64_json(raw)?;
    Ok(LiquidityCursor {
        timestamp: parse_rfc3339(&wire.timestamp)?,
        signature: wire.signature,
    })
}

fn sort_column_to_wire(c: PoolSortColumn) -> &'static str {
    match c {
        PoolSortColumn::FirstSeen => "first_seen",
        PoolSortColumn::LastSeen => "last_seen",
    }
}

fn sort_column_from_wire(raw: &str) -> Result<PoolSortColumn, ApiError> {
    match raw {
        "first_seen" => Ok(PoolSortColumn::FirstSeen),
        "last_seen" => Ok(PoolSortColumn::LastSeen),
        _ => Err(ApiError::BadRequest(
            "invalid cursor: unknown sort column".to_string(),
        )),
    }
}

#[cfg(test)]
#[path = "tests/cursor_tests.rs"]
mod tests;
