use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2ClosePositionEventRepository;

use yog_core::domain::MeteoraDammV2ClosePositionEvent;
use yog_persistence::PgMeteoraDammV2ClosePositionEventRepository;

use super::helpers::{pk, sg, ts};

// ── close_position: persists ─────────────────────────────────────────

#[sqlx::test]
async fn close_position_inserts(pool: PgPool) {
    let repo = PgMeteoraDammV2ClosePositionEventRepository::new(pool.clone());
    assert_eq!(
        repo.insert(&MeteoraDammV2ClosePositionEvent {
            pool_address: pk(1),
            signature: sg(),
            timestamp: ts(),
            slot: 1,
            transaction_index: None,
            event_index: 0,
            owner: pk(2),
            position: pk(3),
            position_nft_mint: pk(4),
        })
        .await
        .unwrap(),
        InsertOutcome::Inserted
    );

    let owner: String =
        sqlx::query_scalar("SELECT owner FROM meteora_damm_v2_close_position_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner, pk(2).to_string());
}
