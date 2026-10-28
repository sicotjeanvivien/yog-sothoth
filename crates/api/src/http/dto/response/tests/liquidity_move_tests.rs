use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::domain::{LiquidityMove, LiquidityMoveKind, LiquidityMoveLock, Protocol};

use super::LiquidityMoveResponse;

fn lock_on_the_wire(lock: Option<LiquidityMoveLock>) -> serde_json::Value {
    let item = LiquidityMove {
        protocol: Protocol::MeteoraDammV2,
        at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        signature: "sig".to_owned(),
        event_index: 0,
        pool_address: Pubkey::new_unique(),
        token_a_symbol: None,
        token_b_symbol: None,
        owner: Pubkey::new_unique(),
        kind: LiquidityMoveKind::Create,
        lock,
        amount_usd: Decimal::ONE,
    };
    serde_json::to_value(LiquidityMoveResponse::from(item)).unwrap()["lock"].clone()
}

/// Mutation: a lock mapped to another, or `none` sent as `null` — a creation without a lock read
/// as one not measured.
#[test]
fn a_move_carries_its_lock_and_null_before_it_was_measured() {
    assert_eq!(lock_on_the_wire(Some(LiquidityMoveLock::None)), "none");
    assert_eq!(
        lock_on_the_wire(Some(LiquidityMoveLock::Vesting)),
        "vesting"
    );
    assert_eq!(
        lock_on_the_wire(Some(LiquidityMoveLock::Permanent)),
        "permanent"
    );
    assert!(lock_on_the_wire(None).is_null());
}
