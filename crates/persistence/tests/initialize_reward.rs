use sqlx::PgPool;
use yog_core::domain::MeteoraDammV2InitializeRewardEventRepository;

use yog_core::domain::MeteoraDammV2InitializeRewardEvent;
use yog_persistence::PgMeteoraDammV2InitializeRewardEventRepository;

use super::helpers::{pk, sg, ts};

// ── initialize_reward: persists, idempotent per slot, multi-slot tx ──

#[sqlx::test]
async fn initialize_reward_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2InitializeRewardEventRepository::new(pool.clone());
    // Real fixture shape: funder == creator, 7-day window on slot 0.
    let event = MeteoraDammV2InitializeRewardEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        reward_mint: pk(2),
        funder: pk(3),
        creator: pk(3),
        reward_index: 0,
        reward_duration: 604_800,
    };

    repo.insert(&event).await.unwrap();
    // Same (signature, reward_index, timestamp) again — ON CONFLICT DO NOTHING.
    repo.insert(&event).await.unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_initialize_reward_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, reward_index, timestamp) must not insert twice"
    );

    // A single transaction may open more than one slot: the same signature with
    // a different reward_index is a distinct event, not a duplicate. This is why
    // reward_index is part of the idempotency key.
    repo.insert(&MeteoraDammV2InitializeRewardEvent {
        reward_index: 1,
        ..event.clone()
    })
    .await
    .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_initialize_reward_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2, "a second slot in the same tx must insert");

    let (mint, funder, creator, duration): (String, String, String, i64) = sqlx::query_as(
        "SELECT reward_mint, funder, creator, reward_duration \
         FROM meteora_damm_v2_initialize_reward_events WHERE reward_index = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mint, pk(2).to_string());
    assert_eq!(funder, pk(3).to_string());
    assert_eq!(creator, pk(3).to_string());
    assert_eq!(duration, 604_800);
}
