//! Tests for the cursor wire format. Mocks and fixtures are local —
//! cursors are self-contained, no repository involved.

use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use super::{
    decode_liquidity_cursor, decode_pool_cursor, decode_swap_cursor, encode_cursor,
    encode_cursor_opt,
};
use crate::http::error::ApiError;
use yog_core::{
    Cursor,
    domain::{MeteoraDammV2LiquidityEventCursor, MeteoraDammV2SwapEventCursor, PoolCursor},
};

// ── Fixtures ────────────────────────────────────────────────────────

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

fn pool_cursor() -> PoolCursor {
    PoolCursor {
        sort_column: yog_core::PoolSortColumn::FirstSeen,
        sort_value: ts(1_700_000_000),
        pool_address: pk(7),
        as_of: None,
    }
}

/// A cursor over the mutable sort column, which carries the traversal's
/// snapshot fence. The `FirstSeen` one above deliberately does not — the
/// round-trip has to preserve both cases.
fn fenced_pool_cursor() -> PoolCursor {
    PoolCursor {
        sort_column: yog_core::PoolSortColumn::LastSeen,
        sort_value: ts(1_700_000_000),
        pool_address: pk(7),
        as_of: Some(ts(1_700_000_900)),
    }
}

fn swap_cursor() -> MeteoraDammV2SwapEventCursor {
    MeteoraDammV2SwapEventCursor {
        timestamp: ts(1_700_000_500),
        signature: sig(1),
        event_index: 3,
    }
}

fn liquidity_cursor() -> MeteoraDammV2LiquidityEventCursor {
    MeteoraDammV2LiquidityEventCursor {
        timestamp: ts(1_700_000_900),
        signature: sig(1),
        event_index: 2,
    }
}

// Helper to assert a result is a BadRequest, regardless of message.
fn assert_bad_request<T: std::fmt::Debug>(result: Result<T, ApiError>) {
    match result {
        Err(ApiError::BadRequest(_)) => {}
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

// ── Round-trips ─────────────────────────────────────────────────────

#[test]
fn pool_cursor_round_trip() {
    let original = pool_cursor();
    let encoded = encode_cursor(&Cursor::Pool(original.clone())).unwrap();
    let decoded = decode_pool_cursor(&encoded).unwrap();
    assert_eq!(decoded, original);
}

/// Losing the fence on the wire would silently un-anchor the traversal: the
/// next page would mint a fresh one and the ordering would move again.
#[test]
fn pool_cursor_round_trip_preserves_the_snapshot_fence() {
    let original = fenced_pool_cursor();
    let encoded = encode_cursor(&Cursor::Pool(original.clone())).unwrap();
    let decoded = decode_pool_cursor(&encoded).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.as_of, original.as_of);
}

/// A cursor minted before the fence existed must still decode — a reader
/// mid-traversal at deploy time keeps navigating, and the repository re-anchors
/// them from the next page on. Rejecting it would 400 a URL that was valid when
/// it was issued.
#[test]
fn pool_cursor_without_as_of_decodes_as_unfenced() {
    let blob = URL_SAFE_NO_PAD_ENCODE(
        r#"{"sort_column":"last_seen","sort_value":"2026-05-25T12:00:00Z",
            "pool_address":"11111111111111111111111111111111"}"#,
    );
    let decoded = decode_pool_cursor(&blob).expect("legacy cursor should still decode");
    assert_eq!(decoded.as_of, None);
}

/// The fence goes through the same total decoding as every other field: a
/// tampered value is a 400, never a panic and never a silently dropped anchor.
#[test]
fn rejects_malformed_as_of() {
    let blob = URL_SAFE_NO_PAD_ENCODE(
        r#"{"sort_column":"last_seen","sort_value":"2026-05-25T12:00:00Z",
            "pool_address":"11111111111111111111111111111111","as_of":"yesterday"}"#,
    );
    assert_bad_request(decode_pool_cursor(&blob));
}

#[test]
fn swap_cursor_round_trip() {
    let original = swap_cursor();
    let encoded = encode_cursor(&Cursor::MeteoraDammV2SwapEvent(original.clone())).unwrap();
    let decoded = decode_swap_cursor(&encoded).unwrap();
    assert_eq!(decoded.timestamp, original.timestamp);
    assert_eq!(decoded.signature, original.signature);
    // Dropping this on the wire would resume a page at the wrong hop of a
    // routed transaction — the third key is what makes the order total.
    assert_eq!(decoded.event_index, original.event_index);
}

#[test]
fn liquidity_cursor_round_trip() {
    let original = liquidity_cursor();
    let encoded = encode_cursor(&Cursor::MeteoraDammV2LiquidityEvent(original.clone())).unwrap();
    let decoded = decode_liquidity_cursor(&encoded).unwrap();
    assert_eq!(decoded.timestamp, original.timestamp);
    assert_eq!(decoded.signature, original.signature);
    assert_eq!(decoded.event_index, original.event_index);
}

#[test]
fn timestamp_survives_round_trip_to_the_second() {
    // RFC3339 carries sub-second precision; verify nothing is lost
    // for a timestamp with a non-zero offset from a round number.
    let original = MeteoraDammV2SwapEventCursor {
        timestamp: Utc.timestamp_opt(1_700_000_123, 0).unwrap(),
        signature: sig(1),
        event_index: 7,
    };
    let encoded = encode_cursor(&Cursor::MeteoraDammV2SwapEvent(original.clone())).unwrap();
    let decoded = decode_swap_cursor(&encoded).unwrap();
    assert_eq!(decoded.timestamp, original.timestamp);
}

// ── encode_cursor_opt ───────────────────────────────────────────────

#[test]
fn encode_cursor_opt_none_yields_none() {
    let result = encode_cursor_opt(None).unwrap();
    assert!(result.is_none());
}

#[test]
fn encode_cursor_opt_some_yields_some() {
    let cursor = Cursor::Pool(pool_cursor());
    let result = encode_cursor_opt(Some(&cursor)).unwrap();
    assert!(result.is_some());
}

// ── Output format ───────────────────────────────────────────────────

#[test]
fn encoded_cursor_is_url_safe_base64() {
    // No '+', '/', or '=' padding — safe to drop straight into a query
    // string without percent-encoding.
    let encoded = encode_cursor(&Cursor::Pool(pool_cursor())).unwrap();
    assert!(!encoded.contains('+'));
    assert!(!encoded.contains('/'));
    assert!(!encoded.contains('='));
    assert!(
        encoded
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
}

// ── Cross-variant safety ────────────────────────────────────────────
//
// The wire format is self-describing: a SwapCursor blob carries
// `timestamp`/`signature`, a PoolCursor blob carries
// `first_seen_at`/`pool_address`. Decoding one as the other must fail
// on the missing fields rather than silently producing garbage.

#[test]
fn pool_blob_does_not_decode_as_swap() {
    let encoded = encode_cursor(&Cursor::Pool(pool_cursor())).unwrap();
    // PoolCursorWire has no `timestamp`/`signature` → serde rejects.
    assert_bad_request(decode_swap_cursor(&encoded));
}

#[test]
fn swap_blob_does_not_decode_as_pool() {
    let encoded = encode_cursor(&Cursor::MeteoraDammV2SwapEvent(swap_cursor())).unwrap();
    // EventCursorWire has no `first_seen_at`/`pool_address` → serde rejects.
    assert_bad_request(decode_pool_cursor(&encoded));
}

// Note: a Swap blob and a Liquidity blob share the same wire shape
// (EventCursorWire), so they are intentionally interchangeable at the
// decode level. The handler picks the right decoder per endpoint;
// there's no field-level discriminator between them, and that's by
// design. We therefore do NOT assert that a swap blob fails to decode
// as a liquidity cursor — it succeeds, and that's expected.

#[test]
fn swap_blob_decodes_as_liquidity_by_shape() {
    // Documents the intentional shape-sharing rather than guarding
    // against it: both event cursors use EventCursorWire.
    let encoded = encode_cursor(&Cursor::MeteoraDammV2SwapEvent(swap_cursor())).unwrap();
    let decoded = decode_liquidity_cursor(&encoded).unwrap();
    assert_eq!(decoded.signature, swap_cursor().signature);
}

// ── Malformed input ─────────────────────────────────────────────────

#[test]
fn rejects_non_base64() {
    // '!' is outside the base64url alphabet.
    assert_bad_request(decode_pool_cursor("not!base64!"));
    assert_bad_request(decode_swap_cursor("not!base64!"));
    assert_bad_request(decode_liquidity_cursor("not!base64!"));
}

#[test]
fn rejects_valid_base64_but_not_json() {
    // "hello" base64url-encoded — decodes to bytes, but not to JSON.
    let garbage = URL_SAFE_NO_PAD_ENCODE("hello");
    assert_bad_request(decode_pool_cursor(&garbage));
}

#[test]
fn rejects_json_with_wrong_shape() {
    // Valid JSON, valid base64, but missing the expected fields.
    let blob = URL_SAFE_NO_PAD_ENCODE(r#"{"foo":"bar"}"#);
    assert_bad_request(decode_pool_cursor(&blob));
    assert_bad_request(decode_swap_cursor(&blob));
}

#[test]
fn rejects_malformed_timestamp() {
    // Correct shape, but `sort_value` is not RFC3339.
    //
    // The blob must carry the *real* wire field names: an earlier version used
    // `first_seen_at`, which serde rejects as a missing field before the
    // timestamp is ever parsed — the test passed without exercising the line it
    // names.
    let blob = URL_SAFE_NO_PAD_ENCODE(
        r#"{"sort_column":"first_seen","sort_value":"yesterday",
            "pool_address":"11111111111111111111111111111111"}"#,
    );
    assert_bad_request(decode_pool_cursor(&blob));
}

#[test]
fn rejects_malformed_pool_address() {
    // Correct shape and timestamp, but the address is not valid base58.
    let blob = URL_SAFE_NO_PAD_ENCODE(
        r#"{"sort_column":"first_seen","sort_value":"2026-05-25T12:00:00Z",
            "pool_address":"not-a-pubkey!"}"#,
    );
    assert_bad_request(decode_pool_cursor(&blob));
}

#[test]
fn rejects_empty_string() {
    // Empty string is valid base64 (empty bytes) but not valid JSON.
    assert_bad_request(decode_pool_cursor(""));
}

// Small helper so the malformed-input tests can build blobs without
// importing the base64 engine at every call site.
#[allow(nonstandard_style)]
fn URL_SAFE_NO_PAD_ENCODE(s: &str) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}
