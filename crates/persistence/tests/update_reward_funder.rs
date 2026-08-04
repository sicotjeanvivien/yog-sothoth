use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2UpdateRewardFunderEventRepository;

use yog_core::domain::MeteoraDammV2UpdateRewardFunderEvent;
use yog_persistence::PgMeteoraDammV2UpdateRewardFunderEventRepository;

use super::helpers::{pk, sg, ts};

// ── update_reward_funder: persists, old/new not swapped ─────────────

#[sqlx::test]
async fn update_reward_funder_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2UpdateRewardFunderEventRepository::new(pool.clone());
    let event = MeteoraDammV2UpdateRewardFunderEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        reward_index: 0,
        old_funder: pk(2),
        new_funder: pk(3),
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_update_reward_funder_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    // Distinct sentinels: a swapped INSERT binding would surface here.
    let (old, new): (String, String) = sqlx::query_as(
        "SELECT old_funder, new_funder FROM meteora_damm_v2_update_reward_funder_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old, pk(2).to_string());
    assert_eq!(new, pk(3).to_string());
}
