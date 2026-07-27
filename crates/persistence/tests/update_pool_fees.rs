use sqlx::PgPool;
use yog_core::domain::MeteoraDammV2UpdatePoolFeesEventRepository;

use yog_core::domain::MeteoraDammV2UpdatePoolFeesEvent;
use yog_persistence::PgMeteoraDammV2UpdatePoolFeesEventRepository;

use super::helpers::{pk, sg, ts};

// ── update_pool_fees: BYTEA params blob ─────────────────────────────

#[sqlx::test]
async fn update_pool_fees_preserves_bytea(pool: PgPool) {
    let repo = PgMeteoraDammV2UpdatePoolFeesEventRepository::new(pool.clone());
    let params = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    repo.insert(&MeteoraDammV2UpdatePoolFeesEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        operator: pk(2),
        params_raw: params.clone(),
    })
    .await
    .unwrap();

    let raw: Vec<u8> = sqlx::query_scalar(
        "SELECT params_raw FROM meteora_damm_v2_update_pool_fees_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(raw, params);
}
