//! Tests for the cursor wire format. Mocks and fixtures are local —
//! cursors are self-contained, no repository involved.

use chrono::{DateTime, TimeZone, Utc};

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::{
    EventCollection, decode_event_cursor, decode_liquidity_move_cursor, decode_signal_cursor,
    encode_cursor, encode_cursor_opt,
};
use crate::http::error::ApiError;
use yog_core::{
    Cursor,
    domain::{
        EventCursor, EventScope, LiquidityMoveCursor, LiquidityMoveOrder, SignalCursor, Window,
    },
};

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

fn signal_cursor() -> SignalCursor {
    SignalCursor {
        triggered_at: ts(1_700_000_123),
        id: 42,
    }
}

fn assert_bad_request<T: std::fmt::Debug>(result: Result<T, ApiError>) {
    match result {
        Err(ApiError::BadRequest(_)) => {}
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[test]
fn signal_cursor_round_trip() {
    let original = signal_cursor();
    let encoded = encode_cursor(&Cursor::Signal(original.clone())).unwrap();
    let decoded = decode_signal_cursor(&encoded).unwrap();
    assert_eq!(decoded, original);
}

/// Mutation: the pool or the window not written to, or not read from, the wire — the
/// service would compare a cursor to its list on a default.
#[test]
fn liquidity_move_cursor_round_trip() {
    let original = LiquidityMoveCursor {
        pool: Pubkey::new_unique(),
        window: Window::Week,
        as_of: ts(1_700_000_400),
        order: LiquidityMoveOrder::At,
        amount_usd: Decimal::new(12_345, 2),
        at: ts(1_700_000_123),
        signature: "sig".to_owned(),
        event_index: 3,
    };
    let encoded = encode_cursor(&Cursor::LiquidityMove(Box::new(original.clone()))).unwrap();
    let decoded = decode_liquidity_move_cursor(&encoded, LiquidityMoveOrder::At).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn encode_cursor_opt_none_yields_none() {
    let result = encode_cursor_opt(None).unwrap();
    assert!(result.is_none());
}

#[test]
fn encode_cursor_opt_some_yields_some() {
    let cursor = Cursor::Signal(signal_cursor());
    let result = encode_cursor_opt(Some(&cursor)).unwrap();
    assert!(result.is_some());
}

#[test]
fn encoded_cursor_is_url_safe_base64() {
    // No '+', '/' or '=': safe in a query string without percent-encoding.
    let encoded = encode_cursor(&Cursor::Signal(signal_cursor())).unwrap();
    assert!(!encoded.contains('+'));
    assert!(!encoded.contains('/'));
    assert!(!encoded.contains('='));
    assert!(
        encoded
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
}

#[test]
fn rejects_non_base64() {
    // '!' is outside the base64url alphabet.
    assert_bad_request(decode_signal_cursor("not!base64!"));
}

#[test]
fn rejects_valid_base64_but_not_json() {
    // "hello" base64url-encoded — decodes to bytes, but not to JSON.
    let garbage = URL_SAFE_NO_PAD_ENCODE("hello");
    assert_bad_request(decode_signal_cursor(&garbage));
}

#[test]
fn rejects_json_with_wrong_shape() {
    // Valid JSON, valid base64, but missing the expected fields.
    let blob = URL_SAFE_NO_PAD_ENCODE(r#"{"foo":"bar"}"#);
    assert_bad_request(decode_signal_cursor(&blob));
}

#[test]
fn rejects_malformed_timestamp() {
    // ⚠️ The real wire field names, or serde rejects a missing field before the timestamp parses.
    let blob = URL_SAFE_NO_PAD_ENCODE(r#"{"triggered_at":"yesterday","id":1}"#);
    assert_bad_request(decode_signal_cursor(&blob));
}

#[test]
fn rejects_empty_string() {
    // Empty string is valid base64 (empty bytes) but not valid JSON.
    assert_bad_request(decode_signal_cursor(""));
}

#[allow(nonstandard_style)]
fn URL_SAFE_NO_PAD_ENCODE(s: &str) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}

/// Mutation: a field of the event cursor dropped or renamed on one side of the wire, or a
/// collection encoded under another's tag — a page read back as another key, or refused.
#[test]
fn event_cursor_round_trip() {
    let cursor = EventCursor {
        scope: EventScope::Pool(Pubkey::new_from_array([3; 32])),
        from: ts(1_700_000_000),
        at: ts(1_700_000_123),
        signature: solana_signature::Signature::from([9; 64]),
        event_index: 7,
    };

    let boxed = || Box::new(cursor.clone());
    for (wrapped, collection) in [
        (Cursor::SwapEvent(boxed()), EventCollection::Swaps),
        (
            Cursor::PositionChangeEvent(boxed()),
            EventCollection::PositionChanges,
        ),
        (
            Cursor::PositionFeeClaimEvent(boxed()),
            EventCollection::PositionFeeClaims,
        ),
        (
            Cursor::ProtocolFeeClaimEvent(boxed()),
            EventCollection::ProtocolFeeClaims,
        ),
        (
            Cursor::RewardClaimEvent(boxed()),
            EventCollection::RewardClaims,
        ),
    ] {
        let encoded = encode_cursor(&wrapped).unwrap();
        let decoded = decode_event_cursor(&encoded, collection, cursor.scope).unwrap();

        assert_eq!(decoded, cursor, "{collection:?}");
    }
}
