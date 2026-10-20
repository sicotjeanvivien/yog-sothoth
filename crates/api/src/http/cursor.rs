// The blob is URL-safe unpadded base64 over a JSON object.
use crate::http::error::ApiError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::{
    Cursor,
    domain::{SignalCursor, TokenActivityCursor},
};

#[derive(Debug, Serialize, Deserialize)]
struct SignalCursorWire {
    triggered_at: String,
    id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenActivityCursorWire {
    last_hour: String,
    volume_usd: String,
    mint: String,
}

pub(crate) fn encode_cursor_opt(cursor: Option<&Cursor>) -> Result<Option<String>, ApiError> {
    cursor.map(encode_cursor).transpose()
}

pub(crate) fn encode_cursor(cursor: &Cursor) -> Result<String, ApiError> {
    match cursor {
        Cursor::Signal(c) => encode_b64_json(&SignalCursorWire {
            triggered_at: c.triggered_at.to_rfc3339(),
            id: c.id,
        }),
        Cursor::TokenActivity(c) => encode_b64_json(&TokenActivityCursorWire {
            last_hour: c.last_hour.to_rfc3339(),
            volume_usd: c.volume_usd.to_string(),
            mint: c.mint.to_string(),
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

pub(crate) fn decode_signal_cursor(raw: &str) -> Result<SignalCursor, ApiError> {
    let wire: SignalCursorWire = decode_b64_json(raw)?;
    Ok(SignalCursor {
        triggered_at: parse_rfc3339(&wire.triggered_at)?,
        id: wire.id,
    })
}

pub(crate) fn decode_token_activity_cursor(raw: &str) -> Result<TokenActivityCursor, ApiError> {
    let wire: TokenActivityCursorWire = decode_b64_json(raw)?;
    Ok(TokenActivityCursor {
        last_hour: parse_rfc3339(&wire.last_hour)?,
        volume_usd: Decimal::from_str(&wire.volume_usd)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed volume".to_string()))?,
        mint: Pubkey::from_str(&wire.mint)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed mint".to_string()))?,
    })
}

#[cfg(test)]
#[path = "tests/cursor_tests.rs"]
mod tests;
