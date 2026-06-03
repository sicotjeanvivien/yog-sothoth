//! Unit tests for `TryFrom<PoolCurrentStateRow> for PoolCurrentState`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right state, that each fallible field has its
//! own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use solana_signature::Signature;
use sqlx::types::BigDecimal;
use yog_core::{
    RepositoryError,
    domain::{LastEventKind, PoolCurrentState},
};

use super::PoolCurrentStateRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_PROTOCOL: &str = "meteora_damm_v2";

fn valid_row() -> PoolCurrentStateRow {
    let now = Utc::now();
    PoolCurrentStateRow {
        pool_address: VALID_POOL.into(),
        protocol: VALID_PROTOCOL.into(),
        last_event_at: now,
        last_event_kind: LastEventKind::Swap.as_str().to_string(),
        last_signature: sig(1).to_string(),
        reserve_a: 10_000_000,
        reserve_b: 20_000_000,
        last_sqrt_price: Some(BigDecimal::from(42_000_000_u64)),
        last_swap_at: Some(now),
        liquidity: Some(BigDecimal::from(100_000_000_u64)),
        last_liquidity_at: Some(now - Duration::seconds(60)),
        updated_at: now + Duration::seconds(1),
    }
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_state_with_all_fields_mapped() {
    let row = valid_row();
    // Snapshot timestamp values before consuming the row.
    let last_event_at = row.last_event_at;
    let last_swap_at = row.last_swap_at;
    let last_liquidity_at = row.last_liquidity_at;
    let updated_at = row.updated_at;

    let state = PoolCurrentState::try_from(row).expect("valid row should convert");

    assert_eq!(state.pool_address.to_string(), VALID_POOL);
    assert_eq!(state.protocol.as_str(), VALID_PROTOCOL);
    assert_eq!(state.last_event_at, last_event_at);
    assert_eq!(state.last_event_kind, LastEventKind::Swap);
    assert_eq!(state.last_signature.to_string(), sig(1).to_string());
    assert_eq!(state.reserve_a, 10_000_000);
    assert_eq!(state.reserve_b, 20_000_000);
    assert_eq!(state.last_sqrt_price, Some(42_000_000_u128));
    assert_eq!(state.last_swap_at, last_swap_at);
    assert_eq!(state.liquidity, Some(100_000_000_u128));
    assert_eq!(state.last_liquidity_at, last_liquidity_at);
    assert_eq!(state.updated_at, updated_at);
}

#[test]
fn try_from_with_none_optionals_returns_state_with_none() {
    // The four Option fields go through `.map(...).transpose()?`;
    // pin that None inputs produce None outputs (and not, say,
    // `Some(0)` from a misplaced `unwrap_or_default`).
    let row = PoolCurrentStateRow {
        last_sqrt_price: None,
        last_swap_at: None,
        liquidity: None,
        last_liquidity_at: None,
        ..valid_row()
    };

    let state = PoolCurrentState::try_from(row).expect("None optionals should convert");

    assert_eq!(state.last_sqrt_price, None);
    assert_eq!(state.last_swap_at, None);
    assert_eq!(state.liquidity, None);
    assert_eq!(state.last_liquidity_at, None);
}

#[test]
fn try_from_preserves_distinct_timestamps_in_correct_fields() {
    // Four distinct timestamp values — a field swap in the TryFrom
    // would surface as a mismatched assert below.
    let last_event_at = Utc::now();
    let last_swap_at = last_event_at + Duration::seconds(10);
    let last_liquidity_at = last_event_at + Duration::seconds(20);
    let updated_at = last_event_at + Duration::seconds(30);
    let row = PoolCurrentStateRow {
        last_event_at,
        last_swap_at: Some(last_swap_at),
        last_liquidity_at: Some(last_liquidity_at),
        updated_at,
        ..valid_row()
    };

    let state = PoolCurrentState::try_from(row).expect("valid row should convert");

    assert_eq!(state.last_event_at, last_event_at);
    assert_eq!(state.last_swap_at, Some(last_swap_at));
    assert_eq!(state.last_liquidity_at, Some(last_liquidity_at));
    assert_eq!(state.updated_at, updated_at);
}

// ── Enum validation ──────────────────────────────────────────────────

#[test]
fn try_from_invalid_last_event_kind_returns_integrity_with_value_in_message() {
    let row = PoolCurrentStateRow {
        last_event_kind: "definitely_not_a_kind".into(),
        ..valid_row()
    };
    let err = PoolCurrentState::try_from(row).expect_err("unknown kind should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid last_event_kind") && msg.contains("definitely_not_a_kind"),
        "expected message to mention both the field and the bad value, got: {msg}"
    );
}

// ── Numeric conversion: i64 → u64 ────────────────────────────────────

#[test]
fn try_from_negative_reserve_a_returns_integrity() {
    let row = PoolCurrentStateRow {
        reserve_a: -1,
        ..valid_row()
    };
    let err = PoolCurrentState::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_b_returns_integrity() {
    let row = PoolCurrentStateRow {
        reserve_b: -1,
        ..valid_row()
    };
    let err = PoolCurrentState::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── BigDecimal → u128 (only when Some) ───────────────────────────────

#[test]
fn try_from_negative_last_sqrt_price_returns_integrity() {
    let row = PoolCurrentStateRow {
        last_sqrt_price: Some(BigDecimal::from(-1_i64)),
        ..valid_row()
    };
    let err = PoolCurrentState::try_from(row)
        .expect_err("negative BigDecimal should fail u128 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_liquidity_returns_integrity() {
    let row = PoolCurrentStateRow {
        liquidity: Some(BigDecimal::from(-1_i64)),
        ..valid_row()
    };
    let err = PoolCurrentState::try_from(row)
        .expect_err("negative BigDecimal should fail u128 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
