use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository;

use yog_core::domain::MeteoraDammV2WithdrawDeadLiquidityRewardEvent;
use yog_persistence::PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository;

use super::helpers::{pk, sg, ts};

#[sqlx::test]
async fn withdraw_dead_liquidity_reward_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository::new(pool.clone());
    // cp-amm emits this event only for a non-zero amount, unlike the ineligible reward.
    let event = MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        reward_mint: pk(2),
        amount: 42_000,
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    // The identically-shaped ineligible-reward table must be untouched: these
    // are two different on-chain facts and two different tables.
    let other: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM meteora_damm_v2_withdraw_ineligible_reward_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(other, 0, "dead-liquidity rows must not land in 031's table");

    let (mint, amount): (String, i64) = sqlx::query_as(
        "SELECT reward_mint, amount FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mint, pk(2).to_string());
    assert_eq!(amount, 42_000);
}
