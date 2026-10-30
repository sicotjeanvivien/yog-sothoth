//! Tests for the cursor wire format. Mocks and fixtures are local —
//! cursors are self-contained, no repository involved.

use chrono::{DateTime, TimeZone, Utc};

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::{
    EventCollection, decode_announcement_cursor, decode_event_cursor, decode_liquidity_move_cursor,
    decode_protocol_cursor, decode_signal_cursor, encode_cursor, encode_cursor_opt,
};
use crate::http::error::ApiError;
use yog_core::{
    Cursor,
    domain::{
        EventCursor, EventScope, LiquidityMoveCursor, LiquidityMoveKind, LiquidityMoveList,
        LiquidityMoveOrder, LiquidityMoveScope, Protocol, SignalCursor, Window,
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

/// Mutation: the scope, the kind or the window not written to, or not read from, the wire — a
/// page of a pool followed under its protocol, of the creations over every move, compared on a
/// default, or the last hour's refused.
#[test]
fn liquidity_move_cursor_round_trip() {
    for window in [Window::Hour, Window::Week] {
        liquidity_move_cursor_round_trips_over(window);
    }
}

fn liquidity_move_cursor_round_trips_over(window: Window) {
    let scope = LiquidityMoveScope::Pool(Pubkey::new_unique());
    let list = LiquidityMoveList {
        scope,
        kind: Some(LiquidityMoveKind::Create),
    };
    let original = LiquidityMoveCursor {
        list,
        window,
        as_of: ts(1_700_000_400),
        order: LiquidityMoveOrder::At,
        amount_usd: Decimal::new(12_345, 2),
        at: ts(1_700_000_123),
        signature: "sig".to_owned(),
        event_index: 3,
    };
    let encoded = encode_cursor(&Cursor::LiquidityMove(Box::new(original.clone()))).unwrap();
    let decoded = decode_liquidity_move_cursor(&encoded, list, LiquidityMoveOrder::At).unwrap();
    let elsewhere = LiquidityMoveList {
        scope: LiquidityMoveScope::Protocol(Protocol::MeteoraDammV2),
        ..list
    };
    let every_kind = LiquidityMoveList { kind: None, ..list };
    assert_eq!(decoded, original);
    assert!(decode_liquidity_move_cursor(&encoded, elsewhere, LiquidityMoveOrder::At).is_err());
    assert!(decode_liquidity_move_cursor(&encoded, every_kind, LiquidityMoveOrder::At).is_err());
}

/// Mutation: the protocol not written to, or not read from, the wire — every page after the
/// first starting over.
#[test]
fn protocol_cursor_round_trip() {
    for &protocol in Protocol::all() {
        let encoded = encode_cursor(&Cursor::Protocol(protocol)).unwrap();
        assert_eq!(decode_protocol_cursor(&encoded).unwrap(), protocol);
    }
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
        (
            Cursor::RewardChangeEvent(boxed()),
            EventCollection::RewardChanges,
        ),
        (
            Cursor::PoolChangeEvent(boxed()),
            EventCollection::PoolChanges,
        ),
    ] {
        let encoded = encode_cursor(&wrapped).unwrap();
        let decoded = decode_event_cursor(&encoded, collection, cursor.scope).unwrap();

        assert_eq!(decoded, cursor, "{collection:?}");
    }
}

/// Mutation: the status of the cursor dropped or not compared — a page of another status
/// followed, or the key read back as another.
#[test]
fn announcement_cursor_round_trip_and_status() {
    use yog_core::domain::{AnnouncementCursor, AnnouncementStatus};
    let cursor = AnnouncementCursor {
        status: Some(AnnouncementStatus::Open),
        starts_at: ts(1_700_000_000),
        id: 42,
    };

    let encoded = encode_cursor(&Cursor::Announcement(Box::new(cursor.clone()))).unwrap();

    assert_eq!(
        decode_announcement_cursor(&encoded, Some(AnnouncementStatus::Open)).unwrap(),
        cursor
    );
    assert!(decode_announcement_cursor(&encoded, None).is_err());
    assert!(decode_announcement_cursor(&encoded, Some(AnnouncementStatus::Closed)).is_err());
}
