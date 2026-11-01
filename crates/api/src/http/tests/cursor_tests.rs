//! Tests for the cursor wire format. Mocks and fixtures are local —
//! cursors are self-contained, no repository involved.

use chrono::{DateTime, TimeZone, Utc};

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::{
    EventCollection, decode_announcement_cursor, decode_event_cursor, decode_liquidity_move_cursor,
    decode_protocol_cursor, decode_signal_cursor, decode_token_activity_cursor,
    decode_token_cursor, encode_cursor, encode_cursor_opt,
};
use crate::http::error::ApiError;
use yog_core::{
    Cursor,
    domain::{
        EventCursor, EventScope, LiquidityMoveCursor, LiquidityMoveKind, LiquidityMoveList,
        LiquidityMoveOrder, LiquidityMoveScope, Protocol, SignalCursor, TokenActivityCursor,
        TokenCursor, TokenPriceCursor, Window,
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

// ⚠️ With microseconds, as `fetched_at` has them: truncated to the second, a page skips or repeats a row.
const MICROS: i64 = 1_700_000_123_456_789;
const VALID_DATE: &str = "2023-11-14T22:15:23Z";
const VALID_MINT: &str = "11111111111111111111111111111111";

fn token_cursor() -> TokenCursor {
    TokenCursor {
        fetched_at: Utc.timestamp_micros(MICROS).unwrap(),
        mint: Pubkey::new_unique(),
    }
}

fn token_activity_cursor() -> TokenActivityCursor {
    TokenActivityCursor {
        last_hour: Utc.timestamp_micros(MICROS).unwrap(),
        volume_usd: Decimal::new(123_450, 2),
        mint: Pubkey::new_unique(),
    }
}

fn assert_refused_because<T: std::fmt::Debug>(result: Result<T, ApiError>, reason: &str) {
    match result {
        Err(ApiError::BadRequest(message)) => assert_eq!(message, reason),
        other => panic!("expected a refusal because {reason}, got {other:?}"),
    }
}

/// Mutation: the date and the mint swapped, or the date truncated to the second.
#[test]
fn token_cursor_round_trip() {
    let original = token_cursor();
    let encoded = encode_cursor(&Cursor::Token(original.clone())).unwrap();
    assert_eq!(decode_token_cursor(&encoded).unwrap(), original);
}

/// Mutation: the date and the mint swapped, the date truncated to the second, or the volume rounded
/// or stripped of its final zero, which `Decimal`'s equality alone would not see.
#[test]
fn token_activity_cursor_round_trip() {
    let original = token_activity_cursor();
    let encoded = encode_cursor(&Cursor::TokenActivity(original.clone())).unwrap();
    let decoded = decode_token_activity_cursor(&encoded).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.volume_usd.to_string(), "1234.50");
}

/// Mutation: an unreadable date or mint falling back to a default, resuming the list elsewhere.
#[test]
fn a_damaged_token_cursor_is_refused_for_its_reason() {
    let damaged = |date: &str, mint: &str| {
        URL_SAFE_NO_PAD_ENCODE(&format!(r#"{{"fetched_at":"{date}","mint":"{mint}"}}"#))
    };
    assert!(decode_token_cursor(&damaged(VALID_DATE, VALID_MINT)).is_ok());
    assert_refused_because(
        decode_token_cursor(&damaged("yesterday", VALID_MINT)),
        "invalid cursor: malformed timestamp",
    );
    assert_refused_because(
        decode_token_cursor(&damaged(VALID_DATE, "not-a-mint")),
        "invalid cursor: malformed mint",
    );
}

/// Mutation: an unreadable date, volume or mint falling back to a default, resuming the list elsewhere.
#[test]
fn a_damaged_token_activity_cursor_is_refused_for_its_reason() {
    let damaged = |date: &str, volume: &str, mint: &str| {
        URL_SAFE_NO_PAD_ENCODE(&format!(
            r#"{{"last_hour":"{date}","volume_usd":"{volume}","mint":"{mint}"}}"#
        ))
    };
    assert!(decode_token_activity_cursor(&damaged(VALID_DATE, "1.5", VALID_MINT)).is_ok());
    assert_refused_because(
        decode_token_activity_cursor(&damaged("yesterday", "1.5", VALID_MINT)),
        "invalid cursor: malformed timestamp",
    );
    assert_refused_because(
        decode_token_activity_cursor(&damaged(VALID_DATE, "a lot", VALID_MINT)),
        "invalid cursor: malformed volume",
    );
    assert_refused_because(
        decode_token_activity_cursor(&damaged(VALID_DATE, "1.5", "not-a-mint")),
        "invalid cursor: malformed mint",
    );
}

/// Mutation: `deny_unknown_fields` removed from either format; a price cursor holds every field of
/// a token cursor, and would resume the token list at a price's date.
#[test]
fn a_cursor_of_the_other_list_or_with_an_unknown_field_is_refused() {
    let malformed = "invalid cursor: malformed payload";
    let price = encode_cursor(&Cursor::TokenPrice(Box::new(TokenPriceCursor {
        mint: Pubkey::new_unique(),
        from: ts(1_700_000_000),
        fetched_at: ts(1_700_000_123),
    })))
    .unwrap();
    assert_refused_because(decode_token_cursor(&price), malformed);

    let token = encode_cursor(&Cursor::Token(token_cursor())).unwrap();
    let activity = encode_cursor(&Cursor::TokenActivity(token_activity_cursor())).unwrap();
    assert_refused_because(decode_token_activity_cursor(&token), malformed);
    assert_refused_because(decode_token_cursor(&activity), malformed);

    assert_refused_because(
        decode_token_cursor(&URL_SAFE_NO_PAD_ENCODE(&format!(
            r#"{{"fetched_at":"{VALID_DATE}","mint":"{VALID_MINT}","extra":1}}"#
        ))),
        malformed,
    );
    assert_refused_because(
        decode_token_activity_cursor(&URL_SAFE_NO_PAD_ENCODE(&format!(
            r#"{{"last_hour":"{VALID_DATE}","volume_usd":"1","mint":"{VALID_MINT}","extra":1}}"#
        ))),
        malformed,
    );
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
