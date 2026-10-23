//! `PgTokenMetadataRepository`: the two `list_missing_mints_*` feed yog-context's metadata
//! worker, `list_price_candidates` its price worker.

use super::helpers::{pk, price_at};
use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::{MISSING_MINT_DISCOVERY_WINDOW, TokenMetadataRepository};
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
        .list_missing_mints_of_all_pools()
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
    let missing = repo.list_missing_mints_of_all_pools().await.unwrap();

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
        .list_missing_mints_of_all_pools()
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

async fn insert_pool_discovered_at(
    pool: &PgPool,
    addr: &str,
    mints: Option<(&str, &str)>,
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint,
                            first_seen_at, last_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2, $3, $4, $4)",
    )
    .bind(addr)
    .bind(mints.map(|(a, _)| a))
    .bind(mints.map(|(_, b)| b))
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

fn discovery_window_start() -> DateTime<Utc> {
    Utc::now() - MISSING_MINT_DISCOVERY_WINDOW
}

fn sorted(mints: Vec<Pubkey>) -> Vec<String> {
    let mut mints: Vec<String> = mints.into_iter().map(|m| m.to_string()).collect();
    mints.sort();
    mints
}

async fn missing_of_recent_pools(pool: &PgPool) -> Vec<String> {
    let repo = PgTokenMetadataRepository::new(pool.clone());
    sorted(
        repo.list_missing_mints_of_recent_pools(MISSING_MINT_DISCOVERY_WINDOW)
            .await
            .expect("the recent search must not fail"),
    )
}

async fn missing_of_all_pools(pool: &PgPool) -> Vec<String> {
    let repo = PgTokenMetadataRepository::new(pool.clone());
    sorted(repo.list_missing_mints_of_all_pools().await.unwrap())
}

/// Against the discovery bound moved later, or one pool side left out.
#[sqlx::test]
async fn the_mints_of_a_pool_discovered_within_the_window_are_asked(pool: PgPool) {
    let (mint_a, mint_b) = (pk(40).to_string(), pk(41).to_string());
    let at = discovery_window_start() + Duration::minutes(1);
    insert_pool_discovered_at(&pool, &pk(1).to_string(), Some((&mint_a, &mint_b)), at).await;

    let mut expected = vec![mint_a, mint_b];
    expected.sort();
    assert_eq!(missing_of_recent_pools(&pool).await, expected);
}

/// Against the discovery bound moved earlier or dropped, and against a sweep that filters too.
#[sqlx::test]
async fn the_mints_of_a_pool_discovered_before_the_window_wait_for_the_sweep(pool: PgPool) {
    let (mint_a, mint_b) = (pk(40).to_string(), pk(41).to_string());
    let at = discovery_window_start() - Duration::minutes(1);
    insert_pool_discovered_at(&pool, &pk(1).to_string(), Some((&mint_a, &mint_b)), at).await;

    let recent = missing_of_recent_pools(&pool).await;
    assert!(
        recent.is_empty(),
        "discovered before the window, got {recent:?}"
    );

    let mut expected = vec![mint_a, mint_b];
    expected.sort();
    assert_eq!(missing_of_all_pools(&pool).await, expected);
}

/// Against the `token_metadata` restriction dropped from either query.
#[sqlx::test]
async fn a_mint_with_metadata_is_asked_by_neither(pool: PgPool) {
    let (known, unknown) = (pk(40).to_string(), pk(41).to_string());
    let at = discovery_window_start() + Duration::minutes(1);
    insert_pool_discovered_at(&pool, &pk(1).to_string(), Some((&known, &unknown)), at).await;
    insert_metadata(&pool, &known).await;

    assert_eq!(missing_of_recent_pools(&pool).await, vec![unknown.clone()]);
    assert_eq!(missing_of_all_pools(&pool).await, vec![unknown]);
}

/// Against `side.mint IS NOT NULL` dropped from the recent search.
#[sqlx::test]
async fn the_null_mints_of_a_recent_unresolved_pool_are_ignored(pool: PgPool) {
    let (mint_a, mint_b) = (pk(40).to_string(), pk(41).to_string());
    let at = discovery_window_start() + Duration::minutes(1);
    insert_pool_discovered_at(&pool, &pk(1).to_string(), None, at).await;
    insert_pool_discovered_at(&pool, &pk(2).to_string(), Some((&mint_a, &mint_b)), at).await;
    insert_metadata(&pool, &mint_a).await;

    assert_eq!(missing_of_recent_pools(&pool).await, vec![mint_b]);
}

async fn insert_pool_state(pool: &PgPool, addr: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO pool_current_state (
             pool_address, protocol, last_event_at, last_event_kind, last_signature,
             reserve_a, reserve_b, last_slot, last_event_index
         )
         VALUES ($1, 'meteora_damm_v2', $2, 'swap', 'sig', 1, 1, 1, 0)",
    )
    .bind(addr)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn set_last_event(pool: &PgPool, addr: &str, at: DateTime<Utc>) {
    sqlx::query("UPDATE pool_current_state SET last_event_at = $2 WHERE pool_address = $1")
        .bind(addr)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
}

/// The price worker wrote just now, through a mint without metadata: it anchors the price
/// window without being a candidate itself.
async fn prices_written_just_now(pool: &PgPool) {
    price_at(pool, &pk(99).to_string(), "1", Utc::now()).await;
}

async fn candidates(pool: &PgPool) -> Vec<String> {
    let mut mints: Vec<String> = PgTokenMetadataRepository::new(pool.clone())
        .list_price_candidates(WINDOW)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.to_string())
        .collect();
    mints.sort();
    mints
}

async fn seed_known_pair_last_moved_at(
    pool: &PgPool,
    last_event: DateTime<Utc>,
) -> (String, String) {
    let (addr, mint_a, mint_b) = (pk(1).to_string(), pk(30).to_string(), pk(31).to_string());
    insert_resolved_pool(pool, &addr, &mint_a, &mint_b).await;
    insert_pool_state(pool, &addr, last_event).await;
    insert_metadata(pool, &mint_a).await;
    insert_metadata(pool, &mint_b).await;
    (mint_a, mint_b)
}

const WINDOW: Duration = Duration::hours(24);

fn window_start() -> DateTime<Utc> {
    Utc::now() - WINDOW
}

/// Against the activity bound moved later, the activity branch dropped, or one pool side left out.
#[sqlx::test]
async fn a_mint_active_within_the_window_without_a_price_is_asked(pool: PgPool) {
    let since = window_start();
    let (mint_a, mint_b) = seed_known_pair_last_moved_at(&pool, since + Duration::minutes(1)).await;

    let mut expected = vec![mint_a, mint_b];
    expected.sort();
    assert_eq!(candidates(&pool).await, expected);
}

/// Against the price bound moved later, or the price branch dropped.
#[sqlx::test]
async fn a_quiet_mint_priced_within_the_window_is_asked(pool: PgPool) {
    let since = window_start();
    let (mint_a, _) = seed_known_pair_last_moved_at(&pool, since - Duration::minutes(1)).await;
    price_at(&pool, &mint_a, "1", since + Duration::minutes(1)).await;
    prices_written_just_now(&pool).await;

    assert_eq!(candidates(&pool).await, vec![mint_a]);
}

/// Against either bound moved earlier.
#[sqlx::test]
async fn a_quiet_mint_whose_price_is_older_than_the_window_is_not_asked(pool: PgPool) {
    let since = window_start();
    let (mint_a, _) = seed_known_pair_last_moved_at(&pool, since - Duration::minutes(1)).await;
    price_at(&pool, &mint_a, "1", since - Duration::minutes(1)).await;
    prices_written_just_now(&pool).await;

    let asked = candidates(&pool).await;
    assert!(asked.is_empty(), "quiet on both counts, got {asked:?}");
}

/// Against a mint that, once out, stays out: a new event on its pool brings it back.
#[sqlx::test]
async fn a_mint_out_of_the_window_is_asked_again_after_an_event_on_its_pool(pool: PgPool) {
    let since = window_start();
    let (mint_a, mint_b) = seed_known_pair_last_moved_at(&pool, since - Duration::minutes(1)).await;
    price_at(&pool, &mint_a, "1", since - Duration::minutes(1)).await;
    prices_written_just_now(&pool).await;
    assert!(candidates(&pool).await.is_empty());

    set_last_event(&pool, &pk(1).to_string(), Utc::now()).await;

    let mut expected = vec![mint_a, mint_b];
    expected.sort();
    assert_eq!(candidates(&pool).await, expected);
}

/// Against the price window counted from now, or from the oldest price: after an outage longer
/// than the window, the mints priced at the last write come back, the older ones do not.
#[sqlx::test]
async fn after_an_outage_the_price_window_counts_back_from_the_last_write(pool: PgPool) {
    let last_write = Utc::now() - Duration::days(3);
    let (fresh_then, stale_then) = seed_known_pair_last_moved_at(&pool, last_write).await;
    price_at(&pool, &fresh_then, "1", last_write).await;
    price_at(
        &pool,
        &stale_then,
        "1",
        last_write - WINDOW - Duration::minutes(1),
    )
    .await;

    assert_eq!(candidates(&pool).await, vec![fresh_then]);
}

/// Against an anchor not capped at now: one future-dated price would empty the price window.
#[sqlx::test]
async fn a_future_dated_price_does_not_move_the_window(pool: PgPool) {
    let since = window_start();
    let (mint_a, _) = seed_known_pair_last_moved_at(&pool, since - Duration::minutes(1)).await;
    price_at(&pool, &mint_a, "1", Utc::now() - Duration::hours(1)).await;
    price_at(
        &pool,
        &pk(98).to_string(),
        "1",
        Utc::now() + Duration::days(2),
    )
    .await;

    assert_eq!(candidates(&pool).await, vec![mint_a]);
}

/// Against the `token_metadata` restriction dropped: a price never exists without its metadata row.
#[sqlx::test]
async fn an_active_mint_without_metadata_is_not_asked(pool: PgPool) {
    let since = window_start();
    let (addr, known, unknown) = (pk(1).to_string(), pk(30).to_string(), pk(31).to_string());
    insert_resolved_pool(&pool, &addr, &known, &unknown).await;
    insert_pool_state(&pool, &addr, since + Duration::minutes(1)).await;
    insert_metadata(&pool, &known).await;

    assert_eq!(candidates(&pool).await, vec![known]);
}
