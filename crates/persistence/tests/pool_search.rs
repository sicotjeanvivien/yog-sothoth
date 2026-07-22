//! Integration tests for the `/pools` search filter — single free-text
//! term vs. the `SOL/USDC` token-pair syntax.
//!
//! Gated behind `integration-tests`: they need a live Postgres (sqlx::test
//! provisions an isolated, migrated DB per test). These exercise what the
//! Couche-1 unit tests in `pool/query_tests.rs` cannot — that the assembled
//! search SQL actually runs against a real dataset and filters correctly,
//! including case-insensitivity and pair order-independence.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::{
    PageDirection, PoolSort,
    domain::{Pool, PoolCatalog, PoolListQuery, Protocol},
};
use yog_persistence::PgPoolRepository;

// ── Seed helpers ────────────────────────────────────────────────────

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

/// Insert a token's metadata so the search join has a symbol/name to match.
async fn seed_token(pool: &PgPool, mint: Pubkey, symbol: &str, name: &str) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, symbol, name, decimals, fetched_at, last_refresh_at)
         VALUES ($1, $2, $3, 9, now(), now())",
    )
    .bind(mint.to_string())
    .bind(symbol)
    .bind(name)
    .execute(pool)
    .await
    .expect("seed token failed");
}

/// Insert a pool with explicit token mints (which side is A vs B matters for
/// the pair order-independence test). Timestamps are derived from `seq` so
/// the default LastSeenDesc order is deterministic.
async fn seed_pool(pool: &PgPool, addr: Pubkey, mint_a: Pubkey, mint_b: Pubkey, seq: i64) {
    sqlx::query(
        r#"
        INSERT INTO pools
            (pool_address, protocol, token_a_mint, token_b_mint,
             first_seen_at, last_seen_at)
        VALUES ($1, $2, $3, $4, $5, $5)
        "#,
    )
    .bind(addr.to_string())
    .bind(Protocol::MeteoraDammV2.as_str())
    .bind(mint_a.to_string())
    .bind(mint_b.to_string())
    .bind(ts(seq * 100))
    .execute(pool)
    .await
    .expect("seed pool failed");
}

// Fixed mints for the scenario.
const SOL: u8 = 10;
const USDC: u8 = 11;
const USDT: u8 = 12;
const WBTC: u8 = 13;

// Pool addresses.
const P_SOL_USDC: u8 = 1; // token_a = SOL,  token_b = USDC
const P_USDT_SOL: u8 = 2; // token_a = USDT, token_b = SOL  (SOL on side B)
const P_USDC_WBTC: u8 = 3; // token_a = USDC, token_b = WBTC

/// Three pools + their token metadata:
///   P1  SOL / USDC
///   P2  USDT / SOL   (SOL is the *second* mint — pair matching must be
///                     order-independent to still find "SOL/USDT")
///   P3  USDC / WBTC
async fn seed_scenario(pool: &PgPool) {
    seed_token(pool, pk(SOL), "SOL", "Solana").await;
    seed_token(pool, pk(USDC), "USDC", "USD Coin").await;
    seed_token(pool, pk(USDT), "USDT", "Tether USD").await;
    seed_token(pool, pk(WBTC), "WBTC", "Wrapped BTC").await;

    seed_pool(pool, pk(P_SOL_USDC), pk(SOL), pk(USDC), 3).await;
    seed_pool(pool, pk(P_USDT_SOL), pk(USDT), pk(SOL), 2).await;
    seed_pool(pool, pk(P_USDC_WBTC), pk(USDC), pk(WBTC), 1).await;
}

fn addrs(pools: &[Pool]) -> Vec<Pubkey> {
    pools.iter().map(|p| p.pool_address).collect()
}

/// A neutral `PoolListQuery` carrying only the given search term.
fn search_query(term: &str) -> PoolListQuery {
    PoolListQuery {
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        sort: PoolSort::LastSeenDesc,
        search: Some(term.to_string()),
        fee_bps: None,
        limit: 50,
    }
}

// ── Pair filter ─────────────────────────────────────────────────────

#[sqlx::test]
async fn pair_search_matches_only_the_exact_pair(pool: PgPool) {
    seed_scenario(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo.find_paginated(search_query("SOL/USDC")).await.unwrap();

    // Only the SOL/USDC pool: P2 (SOL/USDT) and P3 (USDC/WBTC) each share
    // exactly one token, which is not enough for a pair match.
    assert_eq!(addrs(&page.items), vec![pk(P_SOL_USDC)]);
}

#[sqlx::test]
async fn pair_search_is_order_independent(pool: PgPool) {
    seed_scenario(&pool).await;
    let repo = PgPoolRepository::new(pool);

    // Query order reversed vs. how the pool stores the mints (SOL is token_b
    // in P2). The OR branch must still find it.
    let page = repo.find_paginated(search_query("SOL/USDT")).await.unwrap();

    assert_eq!(addrs(&page.items), vec![pk(P_USDT_SOL)]);
}

#[sqlx::test]
async fn pair_search_is_case_insensitive(pool: PgPool) {
    seed_scenario(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo.find_paginated(search_query("sol/usdc")).await.unwrap();

    assert_eq!(addrs(&page.items), vec![pk(P_SOL_USDC)]);
}

#[sqlx::test]
async fn pair_search_no_common_pool_yields_empty(pool: PgPool) {
    seed_scenario(&pool).await;
    let repo = PgPoolRepository::new(pool);

    // WBTC and USDT never share a pool.
    let page = repo
        .find_paginated(search_query("WBTC/USDT"))
        .await
        .unwrap();

    assert!(page.items.is_empty());
}

// ── Single term (regression: unchanged behaviour) ───────────────────

#[sqlx::test]
async fn single_term_matches_pools_on_either_side(pool: PgPool) {
    seed_scenario(&pool).await;
    let repo = PgPoolRepository::new(pool);

    // "SOL" alone → every pool holding SOL, on either side: P1 and P2, in
    // LastSeenDesc order (P1 seq 3 before P2 seq 2). P3 (USDC/WBTC) excluded.
    let page = repo.find_paginated(search_query("SOL")).await.unwrap();

    assert_eq!(addrs(&page.items), vec![pk(P_SOL_USDC), pk(P_USDT_SOL)]);
}

#[sqlx::test]
async fn blank_side_falls_back_to_single_term(pool: PgPool) {
    seed_scenario(&pool).await;
    let repo = PgPoolRepository::new(pool);

    // "USDC/" collapses to a single-term "USDC" search: P1 and P3.
    let page = repo.find_paginated(search_query("USDC/")).await.unwrap();

    assert_eq!(addrs(&page.items), vec![pk(P_SOL_USDC), pk(P_USDC_WBTC)]);
}
