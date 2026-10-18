//! `PgPoolRepository`'s writes: `last_seen_at` never moves backwards, and never past the
//! database clock, no index covers it, and concurrent discoveries of one pool record it once.

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

#[sqlx::test]
async fn no_index_of_pools_covers_last_seen_at(pool: PgPool) {
    let indexes: Vec<(String, String)> = sqlx::query_as(
        "SELECT indexname::TEXT, indexdef FROM pg_indexes
          WHERE schemaname = 'public' AND tablename = 'pools'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        indexes.iter().any(|(name, _)| name == "pools_pkey"),
        "the lookup must see the indexes of pools, or an empty answer would pass: {indexes:?}"
    );
    let covering: Vec<&str> = indexes
        .iter()
        .filter(|(_, def)| def.contains("last_seen_at"))
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        covering.is_empty(),
        "an index naming last_seen_at, in its key, an expression or its predicate, keeps \
         every rewrite of the column from being HOT: {covering:?}"
    );
}

/// Mutation: `ON CONFLICT (pool_address) DO UPDATE`, whose arbiter is one of the two unique
/// indexes of `pools`: a racer loses on the other one, or deadlocks on it.
#[sqlx::test]
async fn concurrent_discoveries_of_one_pool_neither_fail_nor_deadlock(
    _options: sqlx::postgres::PgPoolOptions,
    connect: sqlx::postgres::PgConnectOptions,
) {
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use yog_core::{RepositoryError, domain::PoolRepository};

    // ⚠️ Fewer racers or rounds, and the old query's deadlock becomes likely instead of certain.
    const RACERS: u32 = 40;
    const ROUNDS: u16 = 500;

    // ⚠️ Not the pool `sqlx::test` hands out, capped at 5 connections; and all opened before
    // the first race, or a racer opening one starts behind the others.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(RACERS)
        .min_connections(RACERS)
        .test_before_acquire(false)
        .connect_with(connect)
        .await
        .unwrap();
    let mut warm = Vec::new();
    for _ in 0..RACERS {
        warm.push(pool.acquire().await.unwrap());
    }
    drop(warm);

    let repo = Arc::new(PgPoolRepository::new(pool.clone()));
    let mut failures = Vec::new();
    for round in 0..ROUNDS {
        let mut address = [7u8; 32];
        address[..2].copy_from_slice(&round.to_le_bytes());
        let barrier = Arc::new(Barrier::new(RACERS as usize));
        let racers: Vec<_> = (0..RACERS)
            .map(|_| {
                let (repo, barrier) = (repo.clone(), barrier.clone());
                let now = Utc::now();
                let discovered = Pool {
                    pool_address: Pubkey::new_from_array(address),
                    protocol: Protocol::MeteoraDammV2,
                    token_a_mint: None,
                    token_b_mint: None,
                    fee_bps: None,
                    first_seen_at: now,
                    last_seen_at: now,
                };
                tokio::spawn(async move {
                    barrier.wait().await;
                    repo.upsert(&discovered).await
                })
            })
            .collect();
        for racer in racers {
            if let Err(err) = racer.await.unwrap() {
                failures.push(err);
            }
        }
    }

    let unique_violations = failures
        .iter()
        .filter(|e| {
            matches!(e, RepositoryError::Conflict(m) if m.contains("pools_pool_address_protocol_key"))
        })
        .count();
    let deadlocks = failures
        .iter()
        .filter(|e| matches!(e, RepositoryError::Backend(m) if m.contains("deadlock detected")))
        .count();
    assert!(
        failures.is_empty(),
        "{unique_violations} unique_violation (23505) and {deadlocks} deadlock (40P01) among \
         {} failures over {ROUNDS} rounds of {RACERS} racers, the first: {:?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pools")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, i64::from(ROUNDS), "one row per pool discovered");
    pool.close().await;
}
