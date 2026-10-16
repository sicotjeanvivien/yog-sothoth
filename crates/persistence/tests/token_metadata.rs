//! `PgTokenMetadataRepository::list_missing_mints`, which feeds yog-context's metadata worker.

use super::helpers::pk;
use chrono::Utc;
use sqlx::PgPool;

use yog_core::domain::TokenMetadataRepository;
use yog_persistence::PgTokenMetadataRepository;

/// A pool discovered from the event stream, both mints still NULL.
async fn insert_unresolved_pool(pool: &PgPool, addr: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', NULL, NULL)",
    )
    .bind(addr)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_resolved_pool(pool: &PgPool, addr: &str, mint_a: &str, mint_b: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(addr)
    .bind(mint_a)
    .bind(mint_b)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_metadata(pool: &PgPool, mint: &str) {
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1, 9, $2, $2)",
    )
    .bind(mint)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: `mint IS NOT NULL` dropped. ⚠️ Needs an empty `token_metadata`: `x NOT IN ()` is
/// TRUE even for NULL, while with one row present the broken query filters it anyway.
#[sqlx::test]
async fn unresolved_mints_are_ignored_when_no_metadata_exists_yet(pool: PgPool) {
    insert_unresolved_pool(&pool, &pk(1).to_string()).await;

    let repo = PgTokenMetadataRepository::new(pool.clone());
    let missing = repo
        .list_missing_mints()
        .await
        .expect("an unresolved pool must not make the query fail — it has nothing to enrich yet");

    assert!(
        missing.is_empty(),
        "a pool whose mints are NULL has no mint to look up, got {missing:?}"
    );
}

/// The same guard over a non-empty `token_metadata`.
#[sqlx::test]
async fn unresolved_mints_are_ignored_alongside_resolved_ones(pool: PgPool) {
    let mint_a = pk(10).to_string();
    let mint_b = pk(11).to_string();

    insert_unresolved_pool(&pool, &pk(1).to_string()).await;
    insert_resolved_pool(&pool, &pk(2).to_string(), &mint_a, &mint_b).await;
    insert_metadata(&pool, &mint_a).await;

    let repo = PgTokenMetadataRepository::new(pool.clone());
    let missing = repo.list_missing_mints().await.unwrap();

    let missing: Vec<String> = missing.into_iter().map(|m| m.to_string()).collect();
    assert_eq!(
        missing,
        vec![mint_b],
        "only the resolved-but-unknown mint is missing: the NULLs carry nothing \
         to enrich and mint_a already has metadata"
    );
}

/// Both mints of a resolved pool are proposed when neither is known, and each
/// appears once however many pools share it — the `UNION` deduplicates.
#[sqlx::test]
async fn both_mints_are_proposed_once_each(pool: PgPool) {
    let mint_a = pk(20).to_string();
    let mint_b = pk(21).to_string();

    insert_resolved_pool(&pool, &pk(1).to_string(), &mint_a, &mint_b).await;
    insert_resolved_pool(&pool, &pk(2).to_string(), &mint_a, &mint_b).await;

    let repo = PgTokenMetadataRepository::new(pool.clone());
    let mut missing: Vec<String> = repo
        .list_missing_mints()
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.to_string())
        .collect();
    missing.sort();

    let mut expected = vec![mint_a, mint_b];
    expected.sort();
    assert_eq!(missing, expected);
}
