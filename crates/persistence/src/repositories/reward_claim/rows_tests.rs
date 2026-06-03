//! Unit tests for `TryFrom<ClaimRewardEventRow> for ClaimRewardEvent`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right event, that each fallible field has its
//! own validation path (including the i16 → u8 narrowing on
//! `reward_index`), and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use solana_signature::Signature;
use yog_core::{
    RepositoryError,
    domain::{ClaimRewardEvent, Protocol},
};

use super::ClaimRewardEventRow;

// Four distinct valid base58 Pubkeys so a field swap surfaces in the
// happy path test (pool / position / owner / mint_reward).
const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_POSITION: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const VALID_OWNER: &str = "11111111111111111111111111111111";
const VALID_MINT_REWARD: &str = "So11111111111111111111111111111111111111112";

fn valid_sig(seed: u8) -> String {
    Signature::from([seed; 64]).to_string()
}

fn valid_row() -> ClaimRewardEventRow {
    ClaimRewardEventRow {
        pool_address: VALID_POOL.into(),
        protocol: Protocol::MeteoraDammV2.as_str().to_string(),
        signature: valid_sig(1),
        timestamp: Utc::now(),
        position: VALID_POSITION.into(),
        owner: VALID_OWNER.into(),
        mint_reward: VALID_MINT_REWARD.into(),
        reward_index: 3,
        total_reward: 1_000_000,
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_event_with_all_fields_mapped() {
    let event = ClaimRewardEvent::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(event.pool_address.to_string(), VALID_POOL);
    assert_eq!(event.protocol, Protocol::MeteoraDammV2);
    assert_eq!(event.signature, Signature::from([1u8; 64]));
    assert_eq!(event.position.to_string(), VALID_POSITION);
    assert_eq!(event.owner.to_string(), VALID_OWNER);
    assert_eq!(event.mint_reward.to_string(), VALID_MINT_REWARD);
    assert_eq!(event.reward_index, 3);
    assert_eq!(event.total_reward, 1_000_000);
}

#[test]
fn try_from_preserves_signature_and_timestamp() {
    let expected_sig = Signature::from([42u8; 64]);
    let timestamp = Utc::now() + Duration::seconds(123);
    let row = ClaimRewardEventRow {
        signature: expected_sig.to_string(),
        timestamp,
        ..valid_row()
    };

    let event = ClaimRewardEvent::try_from(row).expect("valid row should convert");

    assert_eq!(event.signature, expected_sig);
    assert_eq!(event.timestamp, timestamp);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = ClaimRewardEventRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_position_returns_integrity() {
    let row = ClaimRewardEventRow {
        position: "garbage".into(),
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("invalid position should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_owner_returns_integrity() {
    let row = ClaimRewardEventRow {
        owner: "garbage".into(),
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("invalid owner should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_mint_reward_returns_integrity() {
    let row = ClaimRewardEventRow {
        mint_reward: "garbage".into(),
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("invalid mint_reward should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Signature validation ─────────────────────────────────────────────

#[test]
fn try_from_invalid_signature_returns_integrity() {
    let row = ClaimRewardEventRow {
        signature: "not-a-real-signature".into(),
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("invalid signature should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Enum validation ──────────────────────────────────────────────────

#[test]
fn try_from_invalid_protocol_returns_integrity_with_message() {
    let row = ClaimRewardEventRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("unknown protocol should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid protocol"),
        "expected message to mention the failure context, got: {msg}"
    );
}

// ── Numeric conversion: i64 → u64 ────────────────────────────────────

#[test]
fn try_from_negative_total_reward_returns_integrity() {
    let row = ClaimRewardEventRow {
        total_reward: -1,
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── reward_index bounds (i16 → u8) ───────────────────────────────────

#[test]
fn try_from_negative_reward_index_returns_integrity_with_value() {
    let row = ClaimRewardEventRow {
        reward_index: -1,
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row)
        .expect_err("negative reward_index should fail u8 conversion");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid reward_index") && msg.contains("-1"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}

#[test]
fn try_from_reward_index_above_u8_max_returns_integrity_with_value() {
    let row = ClaimRewardEventRow {
        reward_index: 300,
        ..valid_row()
    };
    let err = ClaimRewardEvent::try_from(row)
        .expect_err("reward_index above 255 should fail u8 conversion");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid reward_index") && msg.contains("300"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}
