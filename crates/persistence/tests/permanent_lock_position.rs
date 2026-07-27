use sqlx::PgPool;
use yog_core::domain::MeteoraDammV2PermanentLockPositionEventRepository;

use yog_core::domain::MeteoraDammV2PermanentLockPositionEvent;
use yog_persistence::PgMeteoraDammV2PermanentLockPositionEventRepository;

use super::helpers::{pk, sg, ts};

// ── permanent_lock_position: both u128 fields round-trip exactly ────

#[sqlx::test]
async fn permanent_lock_position_preserves_u128(pool: PgPool) {
    let repo = PgMeteoraDammV2PermanentLockPositionEventRepository::new(pool.clone());
    let lock: u128 = 38_221_888_425_530_974_168_949_248_950_912;
    let total: u128 = 76_443_776_851_061_948_337_898_497_901_824;
    repo.insert(&MeteoraDammV2PermanentLockPositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        position: pk(2),
        lock_liquidity_amount: lock,
        total_permanent_locked_liquidity: total,
    })
    .await
    .unwrap();

    let (got_lock, got_total): (String, String) = {
        let l: String = sqlx::query_scalar(
            "SELECT lock_liquidity_amount::text FROM meteora_damm_v2_permanent_lock_position_events LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let t: String = sqlx::query_scalar(
            "SELECT total_permanent_locked_liquidity::text FROM meteora_damm_v2_permanent_lock_position_events LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        (l, t)
    };
    assert_eq!(got_lock, lock.to_string());
    assert_eq!(got_total, total.to_string());
}
