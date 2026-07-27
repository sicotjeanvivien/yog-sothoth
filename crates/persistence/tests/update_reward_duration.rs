use sqlx::PgPool;
use yog_core::domain::MeteoraDammV2UpdateRewardDurationEventRepository;

use yog_core::domain::MeteoraDammV2UpdateRewardDurationEvent;
use yog_persistence::PgMeteoraDammV2UpdateRewardDurationEventRepository;

use super::helpers::{pk, sg, ts};

// ── update_reward_duration: persists, idempotent per slot ───────────

#[sqlx::test]
async fn update_reward_duration_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2UpdateRewardDurationEventRepository::new(pool.clone());
    // 7 days -> 14 days: a re-pacing that halves the emission rate of every
    // subsequent funding. Distinct old/new values so an inverted pair fails.
    let event = MeteoraDammV2UpdateRewardDurationEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        reward_index: 0,
        old_reward_duration: 604_800,
        new_reward_duration: 1_209_600,
    };

    repo.insert(&event).await.unwrap();
    repo.insert(&event).await.unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_update_reward_duration_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, reward_index, timestamp) must not insert twice"
    );

    // Second slot in the same transaction: distinct event, not a duplicate.
    repo.insert(&MeteoraDammV2UpdateRewardDurationEvent {
        reward_index: 1,
        ..event.clone()
    })
    .await
    .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_update_reward_duration_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2, "a second slot in the same tx must insert");

    let (old, new): (i64, i64) = sqlx::query_as(
        "SELECT old_reward_duration, new_reward_duration FROM meteora_damm_v2_update_reward_duration_events WHERE reward_index = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((old, new), (604_800, 1_209_600));
}
