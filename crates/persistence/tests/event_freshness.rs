//! `PgEventFreshnessRepository` under `yog_api`: the last event among the swaps and the liquidity.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use yog_core::domain::EventFreshnessRepository;
use yog_persistence::PgEventFreshnessRepository;

use super::helpers::{setup_roles_and_connect_as, ts};

async fn repository(pool: &PgPool) -> PgEventFreshnessRepository {
    PgEventFreshnessRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

async fn insert_swap(pool: &PgPool, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ('pool', 'swap', 'a_to_b', 1, 1, 0, 0, 0, 0, 0, 0, 0, true, $1, 0, 0)",
    )
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_liquidity(pool: &PgPool, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
            liquidity_delta, reserve_a_after, reserve_b_after, position, owner,
            timestamp, slot, event_index)
         VALUES ('pool', 'liquidity', 'add', 1, 1, 0::NUMERIC, 0, 0, 'position', 'owner', $1, 0, 0)",
    )
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: no event answered with a date — a new database read as up to date.
#[sqlx::test]
async fn no_event_has_no_date(pool: PgPool) {
    let repo = repository(&pool).await;

    assert_eq!(repo.last_event_at().await.unwrap(), None);
}

/// Mutation: the oldest of the two read, or the liquidity left out — a dot behind the last event.
#[sqlx::test]
async fn the_last_event_can_be_a_liquidity_move(pool: PgPool) {
    let repo = repository(&pool).await;
    insert_swap(&pool, ts()).await;
    insert_liquidity(&pool, ts() + Duration::hours(1)).await;

    assert_eq!(
        repo.last_event_at().await.unwrap(),
        Some(ts() + Duration::hours(1))
    );
}

/// Mutation: the oldest of the two read, or the swaps left out — a dot behind the last event.
#[sqlx::test]
async fn the_last_event_can_be_a_swap(pool: PgPool) {
    let repo = repository(&pool).await;
    insert_swap(&pool, ts() + Duration::hours(1)).await;
    insert_liquidity(&pool, ts()).await;

    assert_eq!(
        repo.last_event_at().await.unwrap(),
        Some(ts() + Duration::hours(1))
    );
}

/// Mutation: the two compared without skipping NULL — ingestion read as stopped while one table is
/// still empty.
#[sqlx::test]
async fn one_table_empty_still_gives_the_date_of_the_other(pool: PgPool) {
    let repo = repository(&pool).await;
    insert_swap(&pool, ts()).await;

    assert_eq!(repo.last_event_at().await.unwrap(), Some(ts()));
}
