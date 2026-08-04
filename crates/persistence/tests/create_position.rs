use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2CreatePositionEventRepository;

use yog_core::domain::MeteoraDammV2CreatePositionEvent;
use yog_persistence::PgMeteoraDammV2CreatePositionEventRepository;

use super::helpers::{pk, sg, ts};

// ── create_position: round trip + idempotency ───────────────────────

#[sqlx::test]
async fn create_position_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2CreatePositionEventRepository::new(pool.clone());
    let event = MeteoraDammV2CreatePositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        owner: pk(2),
        position: pk(3),
        position_nft_mint: pk(4),
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    // Same (signature, timestamp) again — ON CONFLICT DO NOTHING.
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_create_position_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    let nft: String = sqlx::query_scalar(
        "SELECT position_nft_mint FROM meteora_damm_v2_create_position_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(nft, pk(4).to_string());
}
