// The blob is URL-safe unpadded base64 over a JSON object.
// ⚠️ Unknown fields refused: a cursor of another list holding every field of this one would
// otherwise decode as one.
use crate::http::error::ApiError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::{
    Cursor,
    domain::{
        LiquidityMoveCursor, LiquidityMoveOrder, PoolCursor, PoolScope, SignalCursor,
        TokenActivityCursor, TokenCursor, TokenPriceCursor, Window,
    },
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignalCursorWire {
    triggered_at: String,
    id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenActivityCursorWire {
    last_hour: String,
    volume_usd: String,
    mint: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiquidityMoveCursorWire {
    pool: String,
    window: String,
    as_of: String,
    order: String,
    amount_usd: String,
    at: String,
    signature: String,
    event_index: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolCursorWire {
    scope: String,
    first_seen_at: String,
    pool: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenCursorWire {
    fetched_at: String,
    mint: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenPriceCursorWire {
    mint: String,
    from: String,
    fetched_at: String,
}

fn scope_tag(scope: PoolScope) -> String {
    match scope {
        PoolScope::All => "all".to_owned(),
        PoolScope::Protocol(protocol) => format!("protocol:{}", protocol.as_str()),
        PoolScope::Token(mint) => format!("token:{mint}"),
    }
}

fn order_tag(order: LiquidityMoveOrder) -> &'static str {
    match order {
        LiquidityMoveOrder::Amount => "amount",
        LiquidityMoveOrder::At => "at",
    }
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
        Cursor::LiquidityMove(c) => encode_b64_json(&LiquidityMoveCursorWire {
            pool: c.pool.to_string(),
            window: c.window.as_str().to_owned(),
            as_of: c.as_of.to_rfc3339(),
            order: order_tag(c.order).to_owned(),
            amount_usd: c.amount_usd.to_string(),
            at: c.at.to_rfc3339(),
            signature: c.signature.clone(),
            event_index: c.event_index,
        }),
        Cursor::Pool(c) => encode_b64_json(&PoolCursorWire {
            scope: scope_tag(c.scope),
            first_seen_at: c.first_seen_at.to_rfc3339(),
            pool: c.pool_address.to_string(),
        }),
        Cursor::Token(c) => encode_b64_json(&TokenCursorWire {
            fetched_at: c.fetched_at.to_rfc3339(),
            mint: c.mint.to_string(),
        }),
        Cursor::TokenPrice(c) => encode_b64_json(&TokenPriceCursorWire {
            mint: c.mint.to_string(),
            from: c.from.to_rfc3339(),
            fetched_at: c.fetched_at.to_rfc3339(),
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

/// A cursor of a list in `order`; one taken in another order is refused, its key meaningless here.
pub(crate) fn decode_liquidity_move_cursor(
    raw: &str,
    order: LiquidityMoveOrder,
) -> Result<LiquidityMoveCursor, ApiError> {
    let wire: LiquidityMoveCursorWire = decode_b64_json(raw)?;
    if wire.order != order_tag(order) {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another order".to_string(),
        ));
    }
    Ok(LiquidityMoveCursor {
        pool: Pubkey::from_str(&wire.pool)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed pool".to_string()))?,
        window: Window::ALL
            .into_iter()
            .find(|w| w.as_str() == wire.window)
            .ok_or_else(|| ApiError::BadRequest("invalid cursor: malformed window".to_string()))?,
        as_of: parse_rfc3339(&wire.as_of)?,
        order,
        amount_usd: Decimal::from_str(&wire.amount_usd)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed amount".to_string()))?,
        at: parse_rfc3339(&wire.at)?,
        signature: wire.signature,
        event_index: wire.event_index,
    })
}

/// A cursor of the pools of `scope`; one of another scope is refused, its key from another list.
pub(crate) fn decode_pool_cursor(raw: &str, scope: PoolScope) -> Result<PoolCursor, ApiError> {
    let wire: PoolCursorWire = decode_b64_json(raw)?;
    if wire.scope != scope_tag(scope) {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another list".to_string(),
        ));
    }
    Ok(PoolCursor {
        scope,
        first_seen_at: parse_rfc3339(&wire.first_seen_at)?,
        pool_address: Pubkey::from_str(&wire.pool)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed pool".to_string()))?,
    })
}

pub(crate) fn decode_token_cursor(raw: &str) -> Result<TokenCursor, ApiError> {
    let wire: TokenCursorWire = decode_b64_json(raw)?;
    Ok(TokenCursor {
        fetched_at: parse_rfc3339(&wire.fetched_at)?,
        mint: Pubkey::from_str(&wire.mint)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed mint".to_string()))?,
    })
}

/// A cursor of `mint`'s prices; one of another mint is refused.
pub(crate) fn decode_token_price_cursor(
    raw: &str,
    mint: Pubkey,
) -> Result<TokenPriceCursor, ApiError> {
    let wire: TokenPriceCursorWire = decode_b64_json(raw)?;
    if wire.mint != mint.to_string() {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another list".to_string(),
        ));
    }
    Ok(TokenPriceCursor {
        mint,
        from: parse_rfc3339(&wire.from)?,
        fetched_at: parse_rfc3339(&wire.fetched_at)?,
    })
}

#[cfg(test)]
#[path = "tests/cursor_tests.rs"]
mod tests;
