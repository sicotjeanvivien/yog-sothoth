//! Integration tests for the claim continuous aggregates (migrations 012, 013).
//!
//! Gated behind `integration-tests`. History-only rollups with no Rust read
//! path yet, so the tests assert the CA aggregates directly. Real-time
//! aggregation makes the just-inserted rows visible without a manual refresh.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

async fn insert_position_fee(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    fee_a: i64,
    fee_b: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_position_fee_events
           (pool_address, signature, position, owner, fee_a_claimed, fee_b_claimed, timestamp)
         VALUES ($1,$2,'pos','own',$3,$4,$5)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(fee_a)
    .bind(fee_b)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn insert_reward(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    mint_reward: &str,
    reward_index: i16,
    total_reward: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_reward_events
           (pool_address, signature, position, owner, mint_reward, reward_index, total_reward, timestamp)
         VALUES ($1,$2,'pos','own',$3,$4,$5,$6)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(mint_reward)
    .bind(reward_index)
    .bind(total_reward)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn claim_position_fee_cagg_sums_per_pool(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let ts = Utc::now() - Duration::hours(1);

    insert_position_fee(&pool, &pool_addr, "f1", 100, 10, ts).await;
    insert_position_fee(&pool, &pool_addr, "f2", 50, 5, ts).await;

    let (fee_a, fee_b, n): (i64, i64, i64) = sqlx::query_as(
        "SELECT fee_a_claimed::BIGINT, fee_b_claimed::BIGINT, claim_count
         FROM meteora_damm_v2_claim_position_fee_events_hourly
         WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!((fee_a, fee_b, n), (150, 15, 2));
}

#[sqlx::test]
async fn claim_reward_cagg_groups_by_mint(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint_x = pk(2).to_string();
    let mint_y = pk(3).to_string();
    let ts = Utc::now() - Duration::hours(1);

    // Two claims of reward token X, one of token Y — must stay separate rows.
    insert_reward(&pool, &pool_addr, "r1", &mint_x, 0, 1000, ts).await;
    insert_reward(&pool, &pool_addr, "r2", &mint_x, 0, 500, ts).await;
    insert_reward(&pool, &pool_addr, "r3", &mint_y, 1, 700, ts).await;

    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT mint_reward, total_reward::BIGINT, claim_count
         FROM meteora_damm_v2_claim_reward_events_hourly
         WHERE pool_address = $1
         ORDER BY mint_reward",
    )
    .bind(&pool_addr)
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut by_mint: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    for (mint, total, count) in rows {
        by_mint.insert(mint, (total, count));
    }
    assert_eq!(
        by_mint.get(&mint_x),
        Some(&(1500, 2)),
        "reward token X summed"
    );
    assert_eq!(
        by_mint.get(&mint_y),
        Some(&(700, 1)),
        "reward token Y separate"
    );
}
