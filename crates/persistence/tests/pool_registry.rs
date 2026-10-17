//! `PgPoolRepository`'s writes: `last_seen_at` never moves backwards, and never past the
//! database clock.

use super::helpers::pk;
use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::{Pool, Protocol};
use yog_persistence::PgPoolRepository;

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

/// Inserts a pool with explicit timestamps, bypassing `upsert`, which sets both equal.
async fn seed_pool(
    pool: &PgPool,
    addr: Pubkey,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
) {
    sqlx::query(
        r#"
        INSERT INTO pools
            (pool_address, protocol, token_a_mint, token_b_mint,
             first_seen_at, last_seen_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(addr.to_string())
    .bind(Protocol::MeteoraDammV2.as_str())
    .bind(pk(200).to_string())
    .bind(pk(201).to_string())
    .bind(first_seen)
    .bind(last_seen)
    .execute(pool)
    .await
    .expect("seed insert failed");
}

#[sqlx::test]
async fn last_seen_at_never_moves_backwards(pool: PgPool) {
    use yog_core::domain::PoolRepository;

    seed_pool(&pool, pk(1), ts(100), ts(300)).await;
    let repo = PgPoolRepository::new(pool.clone());

    // An older instant than the stored one, as a concurrent touch would leave.
    repo.upsert(&Pool {
        pool_address: pk(1),
        protocol: Protocol::MeteoraDammV2,
        token_a_mint: Some(pk(200)),
        token_b_mint: Some(pk(201)),
        fee_bps: None,
        first_seen_at: ts(100),
        last_seen_at: ts(200),
    })
    .await
    .unwrap();

    assert_eq!(
        last_seen_of(&pool, pk(1)).await,
        ts(300),
        "an older observation must not lower last_seen_at"
    );

    // Mutation: a guard that freezes the column instead of flooring it.
    repo.upsert(&Pool {
        pool_address: pk(1),
        protocol: Protocol::MeteoraDammV2,
        token_a_mint: Some(pk(200)),
        token_b_mint: Some(pk(201)),
        fee_bps: None,
        first_seen_at: ts(100),
        last_seen_at: ts(400),
    })
    .await
    .unwrap();

    assert_eq!(last_seen_of(&pool, pk(1)).await, ts(400));

    repo.touch_last_seen(&pk(1)).await.unwrap();
    assert!(
        last_seen_of(&pool, pk(1)).await > ts(400),
        "a touch at NOW() must still advance the column"
    );
}

async fn last_seen_of(pool: &PgPool, addr: Pubkey) -> DateTime<Utc> {
    sqlx::query_scalar::<_, DateTime<Utc>>("SELECT last_seen_at FROM pools WHERE pool_address = $1")
        .bind(addr.to_string())
        .fetch_one(pool)
        .await
        .expect("read last_seen_at failed")
}

/// Mutation: no clamp to `NOW()` — `GREATEST` would make a future value permanent. ⚠️ Asserts
/// "not in the future": Postgres truncates to microseconds, so `stored < written` holds anyway.
#[sqlx::test]
async fn a_clock_ahead_of_the_database_cannot_store_the_future(pool: PgPool) {
    use yog_core::domain::PoolRepository;

    let repo = PgPoolRepository::new(pool.clone());
    let ahead = Utc::now() + chrono::Duration::hours(3);

    let observed = |at| Pool {
        pool_address: pk(1),
        protocol: Protocol::MeteoraDammV2,
        token_a_mint: Some(pk(200)),
        token_b_mint: Some(pk(201)),
        fee_bps: None,
        first_seen_at: at,
        last_seen_at: at,
    };

    repo.upsert(&observed(ahead)).await.unwrap();
    let ceiling = Utc::now();
    assert!(
        last_seen_of(&pool, pk(1)).await <= ceiling,
        "an insert must not store an instant ahead of the database clock"
    );
    assert!(first_seen_of(&pool, pk(1)).await <= ceiling);

    // On UPDATE, where `GREATEST` would make it permanent.
    repo.upsert(&observed(ahead)).await.unwrap();
    let ceiling = Utc::now();
    let stored = last_seen_of(&pool, pk(1)).await;
    assert!(
        stored <= ceiling,
        "an upsert must not store an instant ahead of the database clock: {stored}"
    );

    // A row already in the future is repaired by its next write.
    sqlx::query("UPDATE pools SET last_seen_at = $2 WHERE pool_address = $1")
        .bind(pk(1).to_string())
        .bind(ahead)
        .execute(&pool)
        .await
        .unwrap();
    repo.upsert(&observed(Utc::now())).await.unwrap();
    let ceiling = Utc::now();
    assert!(
        last_seen_of(&pool, pk(1)).await <= ceiling,
        "a row stuck in the future must come back on its next write"
    );
}

async fn first_seen_of(pool: &PgPool, addr: Pubkey) -> DateTime<Utc> {
    sqlx::query_scalar::<_, DateTime<Utc>>(
        "SELECT first_seen_at FROM pools WHERE pool_address = $1",
    )
    .bind(addr.to_string())
    .fetch_one(pool)
    .await
    .expect("read first_seen_at failed")
}
