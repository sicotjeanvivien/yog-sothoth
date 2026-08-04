use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2ClaimProtocolFeeEventRepository;

use yog_core::domain::MeteoraDammV2ClaimProtocolFeeEvent;
use yog_persistence::PgMeteoraDammV2ClaimProtocolFeeEventRepository;

use super::helpers::{pk, sg, ts};

// ── claim_protocol_fee: persists, idempotent, u64→BIGINT round-trip ──

#[sqlx::test]
async fn claim_protocol_fee_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2ClaimProtocolFeeEventRepository::new(pool.clone());
    // token_a = 0 (nothing claimed on that side), token_b large — the real
    // fixture shape (only one side withdrawn).
    let event = MeteoraDammV2ClaimProtocolFeeEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        token_a_amount: 0,
        token_b_amount: 1_421_627_556,
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    // Same (signature, timestamp) again — ON CONFLICT DO NOTHING.
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_claim_protocol_fee_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    let (a, b): (i64, i64) = sqlx::query_as(
        "SELECT token_a_amount, token_b_amount \
         FROM meteora_damm_v2_claim_protocol_fee_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((a, b), (0, 1_421_627_556));
}
