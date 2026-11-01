//! `insert` of `PgMeteoraDammV2ClaimPositionFeeEventRepository`, read back column by column.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yog_core::domain::{
    InsertOutcome, MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
};
use yog_persistence::PgMeteoraDammV2ClaimPositionFeeEventRepository;

use super::helpers::{pk, sg, ts};

type Row = (
    String,
    String,
    String,
    String,
    i64,
    i64,
    DateTime<Utc>,
    i64,
    i32,
    Option<i64>,
);

fn event() -> MeteoraDammV2ClaimPositionFeeEvent {
    MeteoraDammV2ClaimPositionFeeEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 7,
        transaction_index: Some(8),
        event_index: 9,
        position: pk(2),
        owner: pk(3),
        fee_a_claimed: 5,
        fee_b_claimed: 6,
    }
}

/// Mutation: two binds of one type exchanged — the fee in A written as the fee in B, and it
/// compiles.
#[sqlx::test(migrations = "./migrations")]
async fn each_value_is_written_to_its_column(pool: PgPool) {
    let repo = PgMeteoraDammV2ClaimPositionFeeEventRepository::new(pool.clone());

    let outcome = repo.insert(&event()).await.unwrap();

    assert_eq!(outcome, InsertOutcome::Inserted);
    let row: Row = sqlx::query_as(
        "SELECT pool_address, signature, position, owner, fee_a_claimed, fee_b_claimed,
                timestamp, slot, event_index, transaction_index
           FROM meteora_damm_v2_claim_position_fee_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            pk(1).to_string(),
            sg().to_string(),
            pk(2).to_string(),
            pk(3).to_string(),
            5,
            6,
            ts(),
            7,
            9,
            Some(8),
        )
    );
}

/// Mutation: the `ON CONFLICT` dropped — a replayed transaction fails instead of being skipped.
#[sqlx::test(migrations = "./migrations")]
async fn a_replayed_event_is_skipped(pool: PgPool) {
    let repo = PgMeteoraDammV2ClaimPositionFeeEventRepository::new(pool.clone());

    let first = repo.insert(&event()).await.unwrap();
    let replayed = repo.insert(&event()).await.unwrap();

    assert_eq!(first, InsertOutcome::Inserted);
    assert_eq!(replayed, InsertOutcome::Skipped);
    let rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM meteora_damm_v2_claim_position_fee_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 1);
}
