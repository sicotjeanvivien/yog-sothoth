use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2SetPoolStatusEventRepository;

use yog_core::domain::MeteoraDammV2SetPoolStatusEvent;
use yog_persistence::PgMeteoraDammV2SetPoolStatusEventRepository;

use super::helpers::{pk, sg, ts};

// ── set_pool_status: SMALLINT ───────────────────────────────────────

#[sqlx::test]
async fn set_pool_status_inserts(pool: PgPool) {
    let repo = PgMeteoraDammV2SetPoolStatusEventRepository::new(pool.clone());
    assert_eq!(
        repo.insert(&MeteoraDammV2SetPoolStatusEvent {
            pool_address: pk(1),
            signature: sg(),
            timestamp: ts(),
            slot: 1,
            transaction_index: None,
            event_index: 0,
            status: 1,
        })
        .await
        .unwrap(),
        InsertOutcome::Inserted
    );

    let status: i16 =
        sqlx::query_scalar("SELECT status FROM meteora_damm_v2_set_pool_status_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, 1);
}
