use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2WithdrawIneligibleRewardEventRepository;

use yog_core::domain::MeteoraDammV2WithdrawIneligibleRewardEvent;
use yog_persistence::PgMeteoraDammV2WithdrawIneligibleRewardEventRepository;

use super::helpers::{pk, sg, ts};

// ── withdraw_ineligible_reward: persists, idempotent, zero amount ────

#[sqlx::test]
async fn withdraw_ineligible_reward_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2WithdrawIneligibleRewardEventRepository::new(pool.clone());
    // Real fixture shape: amount is legitimately zero — nothing was reclaimable.
    // NOT NULL + BIGINT must accept it as a value, not treat it as missing.
    let event = MeteoraDammV2WithdrawIneligibleRewardEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        reward_mint: pk(2),
        amount: 0,
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    // Same (signature, timestamp) again — ON CONFLICT DO NOTHING.
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM meteora_damm_v2_withdraw_ineligible_reward_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    let (mint, amount): (String, i64) = sqlx::query_as(
        "SELECT reward_mint, amount \
         FROM meteora_damm_v2_withdraw_ineligible_reward_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mint, pk(2).to_string());
    assert_eq!(amount, 0);
}
