//! `PgPoolRepository`'s catalog reads under `yog_api`: the pools of a scope, newest first,
//! paged without repeat nor gap, each with its tokens, its state and its value.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    Cursor,
    domain::{LastEventKind, PoolCatalog, PoolScope, Protocol},
};
use yog_persistence::PgPoolRepository;

use super::helpers::{DAMM, DLMM, pk, price_at, setup_roles_and_connect_as};

fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::minutes(minutes)
}

async fn repository(pool: &PgPool) -> PgPoolRepository {
    PgPoolRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

async fn insert_pool(
    pool: &PgPool,
    address: Pubkey,
    protocol: &str,
    (mint_a, mint_b): (Pubkey, Pubkey),
    first_seen: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint, fee_bps,
                            first_seen_at, last_seen_at)
         VALUES ($1, $2, $3, $4, 25, $5, $5)",
    )
    .bind(address.to_string())
    .bind(protocol)
    .bind(mint_a.to_string())
    .bind(mint_b.to_string())
    .bind(first_seen)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_metadata(pool: &PgPool, mint: Pubkey, symbol: &str, decimals: i16) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, symbol, decimals, fetched_at, last_refresh_at)
         VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(mint.to_string())
    .bind(symbol)
    .bind(decimals)
    .bind(at(0))
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_state(pool: &PgPool, address: Pubkey, (reserve_a, reserve_b): (i64, i64)) {
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_swap_at, last_slot, last_event_index)
         VALUES ($1, 'meteora_damm_v2', $2, 'liquidity_add', 'sig', $3, $4, NULL, 1, 0)",
    )
    .bind(address.to_string())
    .bind(at(5))
    .bind(reserve_a)
    .bind(reserve_b)
    .execute(pool)
    .await
    .unwrap();
}

fn addresses(page: &yog_core::Page<yog_core::domain::PoolEntry>) -> Vec<Pubkey> {
    page.items.iter().map(|e| e.pool.pool_address).collect()
}

/// Mutation: the protocol filter dropped — DLMM's pool listed under DAMM v2.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_pools_are_its_own(pool: PgPool) {
    insert_pool(&pool, pk(1), DAMM, (pk(10), pk(11)), at(1)).await;
    insert_pool(&pool, pk(2), DLMM, (pk(10), pk(11)), at(2)).await;
    let repository = repository(&pool).await;

    let damm = repository
        .list_entries(PoolScope::Protocol(Protocol::MeteoraDammV2), None, 50)
        .await
        .unwrap();
    let all = repository
        .list_entries(PoolScope::All, None, 50)
        .await
        .unwrap();

    assert_eq!(addresses(&damm), [pk(1)]);
    assert_eq!(addresses(&all), [pk(2), pk(1)]);
}

/// Mutation: one side of the token filter dropped — the pool holding the mint as token B, or
/// as token A, missing.
#[sqlx::test(migrations = "./migrations")]
async fn a_tokens_pools_are_those_it_is_either_side_of(pool: PgPool) {
    let mint = pk(10);
    insert_pool(&pool, pk(1), DAMM, (mint, pk(11)), at(1)).await;
    insert_pool(&pool, pk(2), DAMM, (pk(12), mint), at(2)).await;
    insert_pool(&pool, pk(3), DAMM, (pk(12), pk(11)), at(3)).await;
    let repository = repository(&pool).await;

    let page = repository
        .list_entries(PoolScope::Token(mint), None, 50)
        .await
        .unwrap();

    assert_eq!(addresses(&page), [pk(2), pk(1)]);
}

/// Mutation: the cursor compared on the instant alone, or with `<=` — a pool first seen in the
/// same instant as the last of a page skipped or repeated; or the order ascending.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_without_repeat_nor_gap(pool: PgPool) {
    // Three pairs first seen in the same instant: only the address orders each pair.
    for (seed, minutes) in [(1, 1), (2, 1), (3, 2), (4, 2), (5, 3), (6, 3), (7, 4)] {
        insert_pool(&pool, pk(seed), DAMM, (pk(10), pk(11)), at(minutes)).await;
    }
    let repository = repository(&pool).await;
    let expected = repository
        .list_entries(PoolScope::All, None, 50)
        .await
        .unwrap();

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_entries(PoolScope::All, cursor.take(), 2)
            .await
            .unwrap();
        seen.extend(addresses(&page));
        match page.next_cursor {
            Some(Cursor::Pool(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }

    assert_eq!(seen, addresses(&expected));
    assert_eq!(seen.len(), 7);
    assert_eq!(seen[0], pk(7));
    let firsts: Vec<_> = expected
        .items
        .iter()
        .map(|e| e.pool.first_seen_at)
        .collect();
    assert!(firsts.windows(2).all(|w| w[0] >= w[1]), "{firsts:?}");
}

/// Mutation: a side's metadata, the state or the value joined on the wrong key — a pool reads
/// another's tokens, or loses its state.
#[sqlx::test(migrations = "./migrations")]
async fn an_entry_carries_its_tokens_state_and_value(pool: PgPool) {
    let (mint_a, mint_b) = (pk(10), pk(11));
    insert_pool(&pool, pk(1), DAMM, (mint_a, mint_b), at(1)).await;
    insert_pool(&pool, pk(2), DAMM, (pk(12), pk(13)), at(2)).await;
    insert_metadata(&pool, mint_a, "AAA", 6).await;
    insert_metadata(&pool, mint_b, "BBB", 9).await;
    insert_state(&pool, pk(1), (2_000_000, 3_000_000_000)).await;
    price_at(&pool, &mint_a.to_string(), "1", Utc::now()).await;
    price_at(&pool, &mint_b.to_string(), "2", Utc::now()).await;
    let repository = repository(&pool).await;

    let entry = repository.find_entry(&pk(1)).await.unwrap().unwrap();
    let bare = repository.find_entry(&pk(2)).await.unwrap().unwrap();

    assert_eq!(entry.token_a.unwrap().symbol.as_deref(), Some("AAA"));
    assert_eq!(entry.token_b.unwrap().symbol.as_deref(), Some("BBB"));
    let state = entry.state.unwrap();
    assert_eq!(state.last_event_kind, LastEventKind::LiquidityAdd);
    assert_eq!(
        (state.reserve_a, state.reserve_b),
        (2_000_000, 3_000_000_000)
    );
    assert_eq!(entry.value.value_a_usd, Some(Decimal::from(2)));
    assert_eq!(entry.value.value_b_usd, Some(Decimal::from(6)));
    assert_eq!(entry.value.tvl_usd, Some(Decimal::from(8)));
    assert!(bare.token_a.is_none() && bare.state.is_none() && bare.value.tvl_usd.is_none());
    assert!(repository.find_entry(&pk(3)).await.unwrap().is_none());
}

/// Mutation: a value past a `Decimal` returned as an error — the page fails, and without its
/// cursor every page after it.
#[sqlx::test(migrations = "./migrations")]
async fn a_value_past_a_decimal_is_served_unvalued_with_its_page(pool: PgPool) {
    let (huge, mint_b) = (pk(10), pk(11));
    insert_pool(&pool, pk(1), DAMM, (huge, mint_b), at(1)).await;
    insert_pool(&pool, pk(2), DAMM, (pk(12), pk(13)), at(2)).await;
    insert_metadata(&pool, huge, "HUGE", 0).await;
    insert_metadata(&pool, mint_b, "BBB", 0).await;
    insert_state(&pool, pk(1), (1_000_000_000_000_000_000, 1)).await;
    price_at(&pool, &huge.to_string(), "10000000000000000000", Utc::now()).await;
    price_at(&pool, &mint_b.to_string(), "1", Utc::now()).await;
    let repository = repository(&pool).await;

    let page = repository
        .list_entries(PoolScope::All, None, 50)
        .await
        .unwrap();

    assert_eq!(addresses(&page), [pk(2), pk(1)]);
    let value = page.items[1].value;
    assert_eq!(value.value_a_usd, None);
    assert_eq!(value.value_b_usd, Some(Decimal::ONE));
    assert_eq!(value.tvl_usd, None);
}
