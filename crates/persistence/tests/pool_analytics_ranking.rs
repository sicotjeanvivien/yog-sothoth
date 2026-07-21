//! Integration test for `PgPoolAnalyticsRepository::top_pool_addresses` —
//! specifically the TVL ranking arm, which reads the `pool_current_tvl` VIEW.
//!
//! Gated behind `integration-tests`. Seeds a handful of pools with known TVLs
//! (reserves valued at $1 per unit, 0 decimals, so `tvl = reserve_a`) plus one
//! unpriceable pool (a token with metadata but no price → NULL TVL), and
//! asserts the ranking is by TVL descending, capped by `limit`, with the
//! NULL-TVL pool excluded rather than sorted last.

#![cfg(feature = "integration-tests")]

use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::{PoolAnalyticsRepository, PoolRankMetric};
use yog_persistence::PgPoolAnalyticsRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// Register a token with metadata (0 decimals) and, optionally, a $1 price.
/// Omitting the price is how we build an unpriceable pool.
async fn seed_token(pool: &PgPool, mint: &str, priced: bool) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1, 0, NOW(), NOW())
         ON CONFLICT (mint) DO NOTHING",
    )
    .bind(mint)
    .execute(pool)
    .await
    .unwrap();

    if priced {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
             VALUES ($1, 1.0, 'jupiter', NOW())",
        )
        .bind(mint)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Seed a pool holding `reserve_a` units of token A and nothing of token B.
/// With $1 / 0-decimal tokens, TVL = `reserve_a` (or NULL if a side is
/// unpriced).
async fn seed_pool(pool: &PgPool, addr: Pubkey, mint_a: &str, mint_b: &str, reserve_a: i64) {
    let addr = addr.to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&addr)
    .bind(mint_a)
    .bind(mint_b)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b)
         VALUES ($1, 'meteora_damm_v2', NOW(), 'liquidity_add', 'sig', $2, 0)",
    )
    .bind(&addr)
    .bind(reserve_a)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn tvl_ranking_orders_by_depth_excludes_unpriceable_and_caps(pool: PgPool) {
    let priced = pk(10).to_string();
    let unpriced = pk(11).to_string();
    seed_token(&pool, &priced, true).await;
    seed_token(&pool, &unpriced, false).await;

    // TVLs: A=$300, B=$100, C=$200; D is unpriceable (token B has no price).
    seed_pool(&pool, pk(1), &priced, &priced, 300).await;
    seed_pool(&pool, pk(2), &priced, &priced, 100).await;
    seed_pool(&pool, pk(3), &priced, &priced, 200).await;
    seed_pool(&pool, pk(4), &priced, &unpriced, 999).await;

    let repo = PgPoolAnalyticsRepository::new(pool);

    // Full ranking: A(300), C(200), B(100). D excluded (NULL TVL).
    let ranked = repo
        .top_pool_addresses(PoolRankMetric::Tvl, 10)
        .await
        .unwrap();
    assert_eq!(ranked, vec![pk(1), pk(3), pk(2)]);

    // The cap keeps only the deepest two.
    let top2 = repo
        .top_pool_addresses(PoolRankMetric::Tvl, 2)
        .await
        .unwrap();
    assert_eq!(top2, vec![pk(1), pk(3)]);
}
