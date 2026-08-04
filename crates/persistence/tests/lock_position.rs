use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2LockPositionEventRepository;

use yog_core::domain::MeteoraDammV2LockPositionEvent;
use yog_persistence::PgMeteoraDammV2LockPositionEventRepository;

use super::helpers::{pk, sg, ts};

// ── lock_position: u128 NUMERIC precision at the u128::MAX boundary ──

#[sqlx::test]
async fn lock_position_preserves_u128_and_u16(pool: PgPool) {
    let repo = PgMeteoraDammV2LockPositionEventRepository::new(pool.clone());
    assert_eq!(
        repo.insert(&MeteoraDammV2LockPositionEvent {
            pool_address: pk(1),
            signature: sg(),
            timestamp: ts(),
            slot: 1,
            transaction_index: None,
            event_index: 0,
            position: pk(2),
            owner: pk(3),
            vesting: pk(4),
            cliff_point: 426_006_082,
            period_frequency: 1,
            // u128::MAX is exactly 39 digits — the NUMERIC(39, 0) boundary.
            cliff_unlock_liquidity: u128::MAX,
            liquidity_per_period: 0,
            number_of_period: 65_535, // u16::MAX — must survive the INTEGER column
        })
        .await
        .unwrap(),
        InsertOutcome::Inserted
    );

    let cliff: String = sqlx::query_scalar(
        "SELECT cliff_unlock_liquidity::text FROM meteora_damm_v2_lock_position_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        cliff,
        u128::MAX.to_string(),
        "u128::MAX lost precision in NUMERIC"
    );

    let n: i32 = sqlx::query_scalar(
        "SELECT number_of_period FROM meteora_damm_v2_lock_position_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 65_535);
}
