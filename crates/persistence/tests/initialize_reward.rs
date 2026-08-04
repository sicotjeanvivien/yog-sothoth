use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
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
        slot: 1,
        transaction_index: None,
        event_index: 0,
        reward_mint: pk(2),
        funder: pk(3),
        creator: pk(3),
        reward_index: 0,
        reward_duration: 604_800,
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    // Same (signature, reward_index, timestamp) again — ON CONFLICT DO NOTHING.
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_initialize_reward_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    // A single transaction may open more than one slot. Each opening is its own
    // emission, so what separates them is `event_index` — since migration 041
    // that is the key's discriminant, in place of `reward_index`, and it works
    // the same way for the fifteen event kinds that never had one.
    assert_eq!(
        repo.insert(&MeteoraDammV2InitializeRewardEvent {
            reward_index: 1,
            event_index: 1,
            ..event.clone()
        })
        .await
        .unwrap(),
        InsertOutcome::Inserted
    );

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
