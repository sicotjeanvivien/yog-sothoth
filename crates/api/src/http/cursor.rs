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
        AnnouncementCursor, AnnouncementStatus, EventCursor, EventScope, LiquidityMoveCursor,
        LiquidityMoveKind, LiquidityMoveList, LiquidityMoveOrder, LiquidityMoveScope, PoolCursor,
        PoolScope, Protocol, RotationRunCursor, RotationRunOrder, RotationRunScope, SignalCursor,
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
    scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
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
struct ProtocolCursorWire {
    protocol: String,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RotationRunCursorWire {
    scope: String,
    window: String,
    as_of: String,
    order: String,
    volume_usd: String,
    started_at: String,
    pool: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnnouncementCursorWire {
    status: Option<String>,
    starts_at: String,
    id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventCursorWire {
    collection: String,
    scope: String,
    from: String,
    at: String,
    signature: String,
    event_index: u16,
}

/// The on-chain collections a page of events is read from: one shape of cursor for all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventCollection {
    Swaps,
    LiquidityChanges,
    PoolCreations,
    PositionChanges,
    PositionFeeClaims,
    ProtocolFeeClaims,
    RewardClaims,
    RewardChanges,
    PoolChanges,
}

impl EventCollection {
    fn tag(self) -> &'static str {
        match self {
            Self::Swaps => "swaps",
            Self::LiquidityChanges => "liquidity-changes",
            Self::PoolCreations => "pool-creations",
            Self::PositionChanges => "position-changes",
            Self::PositionFeeClaims => "position-fee-claims",
            Self::ProtocolFeeClaims => "protocol-fee-claims",
            Self::RewardClaims => "reward-claims",
            Self::RewardChanges => "reward-changes",
            Self::PoolChanges => "pool-changes",
        }
    }
}

fn event_scope_tag(scope: EventScope) -> String {
    match scope {
        EventScope::All => "all".to_owned(),
        EventScope::Protocol(protocol) => format!("protocol:{}", protocol.as_str()),
        EventScope::Pool(pool) => format!("pool:{pool}"),
    }
}

fn encode_event_cursor(collection: EventCollection, c: &EventCursor) -> Result<String, ApiError> {
    encode_b64_json(&EventCursorWire {
        collection: collection.tag().to_owned(),
        scope: event_scope_tag(c.scope),
        from: c.from.to_rfc3339(),
        at: c.at.to_rfc3339(),
        signature: c.signature.to_string(),
        event_index: c.event_index,
    })
}

fn scope_tag(scope: PoolScope) -> String {
    match scope {
        PoolScope::All => "all".to_owned(),
        PoolScope::Protocol(protocol) => format!("protocol:{}", protocol.as_str()),
        PoolScope::Token(mint) => format!("token:{mint}"),
    }
}

fn liquidity_move_scope_tag(scope: LiquidityMoveScope) -> String {
    match scope {
        LiquidityMoveScope::Protocol(protocol) => format!("protocol:{}", protocol.as_str()),
        LiquidityMoveScope::Pool(pool) => format!("pool:{pool}"),
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
            scope: liquidity_move_scope_tag(c.list.scope),
            kind: c.list.kind.map(|k| k.as_str().to_owned()),
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
        Cursor::SwapEvent(c) => encode_event_cursor(EventCollection::Swaps, c),
        Cursor::LiquidityChangeEvent(c) => {
            encode_event_cursor(EventCollection::LiquidityChanges, c)
        }
        Cursor::PoolCreationEvent(c) => encode_event_cursor(EventCollection::PoolCreations, c),
        Cursor::PositionChangeEvent(c) => encode_event_cursor(EventCollection::PositionChanges, c),
        Cursor::PositionFeeClaimEvent(c) => {
            encode_event_cursor(EventCollection::PositionFeeClaims, c)
        }
        Cursor::ProtocolFeeClaimEvent(c) => {
            encode_event_cursor(EventCollection::ProtocolFeeClaims, c)
        }
        Cursor::RewardClaimEvent(c) => encode_event_cursor(EventCollection::RewardClaims, c),
        Cursor::RewardChangeEvent(c) => encode_event_cursor(EventCollection::RewardChanges, c),
        Cursor::PoolChangeEvent(c) => encode_event_cursor(EventCollection::PoolChanges, c),
        Cursor::RotationRun(c) => encode_b64_json(&RotationRunCursorWire {
            scope: rotation_run_scope_tag(c.scope),
            window: c.window.as_str().to_owned(),
            as_of: c.as_of.to_rfc3339(),
            order: rotation_run_order_tag(c.order).to_owned(),
            volume_usd: c.volume_usd.to_string(),
            started_at: c.started_at.to_rfc3339(),
            pool: c.pool_address.to_string(),
        }),
        Cursor::Announcement(c) => encode_b64_json(&AnnouncementCursorWire {
            status: c.status.map(|s| s.as_str().to_owned()),
            starts_at: c.starts_at.to_rfc3339(),
            id: c.id,
        }),
        Cursor::Protocol(protocol) => encode_b64_json(&ProtocolCursorWire {
            protocol: protocol.as_str().to_owned(),
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

/// A cursor of `list`'s moves in `order`; one of another scope, kind or order is refused, its
/// key meaningless here.
pub(crate) fn decode_liquidity_move_cursor(
    raw: &str,
    list: LiquidityMoveList,
    order: LiquidityMoveOrder,
) -> Result<LiquidityMoveCursor, ApiError> {
    let wire: LiquidityMoveCursorWire = decode_b64_json(raw)?;
    if wire.scope != liquidity_move_scope_tag(list.scope)
        || wire.kind.as_deref() != list.kind.map(LiquidityMoveKind::as_str)
    {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another list".to_string(),
        ));
    }
    if wire.order != order_tag(order) {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another order".to_string(),
        ));
    }
    Ok(LiquidityMoveCursor {
        list,
        window: [Window::Hour]
            .into_iter()
            .chain(Window::ALL)
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

fn rotation_run_scope_tag(scope: RotationRunScope) -> String {
    match scope {
        RotationRunScope::Protocol(protocol) => format!("protocol:{}", protocol.as_str()),
        RotationRunScope::Pool(pool) => format!("pool:{pool}"),
    }
}

fn rotation_run_order_tag(order: RotationRunOrder) -> &'static str {
    match order {
        RotationRunOrder::Volume => "volume",
        RotationRunOrder::At => "at",
    }
}

/// A cursor of `scope`'s runs over `window` in `order`; one of another list is refused.
pub(crate) fn decode_rotation_run_cursor(
    raw: &str,
    scope: RotationRunScope,
    window: Window,
    order: RotationRunOrder,
) -> Result<RotationRunCursor, ApiError> {
    let wire: RotationRunCursorWire = decode_b64_json(raw)?;
    if wire.scope != rotation_run_scope_tag(scope)
        || wire.window != window.as_str()
        || wire.order != rotation_run_order_tag(order)
    {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another list".to_string(),
        ));
    }
    Ok(RotationRunCursor {
        scope,
        window,
        as_of: parse_rfc3339(&wire.as_of)?,
        order,
        volume_usd: Decimal::from_str(&wire.volume_usd)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed volume".to_string()))?,
        started_at: parse_rfc3339(&wire.started_at)?,
        pool_address: Pubkey::from_str(&wire.pool)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed pool".to_string()))?,
    })
}

/// A cursor of the protocol catalogue; a protocol this build does not know is refused.
pub(crate) fn decode_protocol_cursor(raw: &str) -> Result<Protocol, ApiError> {
    let wire: ProtocolCursorWire = decode_b64_json(raw)?;
    Protocol::all()
        .iter()
        .copied()
        .find(|protocol| protocol.as_str() == wire.protocol)
        .ok_or_else(|| ApiError::BadRequest("invalid cursor: unknown protocol".to_string()))
}

/// A cursor of the announcements of `status`; one taken under another status is refused.
pub(crate) fn decode_announcement_cursor(
    raw: &str,
    status: Option<AnnouncementStatus>,
) -> Result<AnnouncementCursor, ApiError> {
    let wire: AnnouncementCursorWire = decode_b64_json(raw)?;
    if wire.status.as_deref() != status.map(|s| s.as_str()) {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken under another status".to_string(),
        ));
    }
    Ok(AnnouncementCursor {
        status,
        starts_at: parse_rfc3339(&wire.starts_at)?,
        id: wire.id,
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

/// A cursor of `collection`'s events in `scope`; one of another collection or scope is refused.
pub(crate) fn decode_event_cursor(
    raw: &str,
    collection: EventCollection,
    scope: EventScope,
) -> Result<EventCursor, ApiError> {
    let wire: EventCursorWire = decode_b64_json(raw)?;
    if wire.collection != collection.tag() || wire.scope != event_scope_tag(scope) {
        return Err(ApiError::BadRequest(
            "invalid cursor: taken in another list".to_string(),
        ));
    }
    Ok(EventCursor {
        scope,
        from: parse_rfc3339(&wire.from)?,
        at: parse_rfc3339(&wire.at)?,
        signature: solana_signature::Signature::from_str(&wire.signature)
            .map_err(|_| ApiError::BadRequest("invalid cursor: malformed signature".to_string()))?,
        event_index: wire.event_index,
    })
}

#[cfg(test)]
#[path = "tests/cursor_tests.rs"]
mod tests;
