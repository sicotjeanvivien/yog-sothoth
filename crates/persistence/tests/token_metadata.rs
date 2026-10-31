//! `PgTokenMetadataRepository`: the two `list_missing_mints_*` feed yog-context's metadata
//! worker, `list_price_candidates` its price worker.

use super::helpers::{pk, price_at};
use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::{
    MISSING_MINT_DISCOVERY_WINDOW, PRICE_CANDIDATE_PARAMS, PriceCandidateClass,
    TokenMetadataRepository,
};
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

/// Mutation: `mint IS NOT NULL` dropped.
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

/// Mutation: `NOT IN` back in the sweep.
#[sqlx::test]
async fn the_sweep_reads_token_metadata_as_an_anti_join(pool: PgPool) {
    let explain = concat!(
        "EXPLAIN ",
        include_str!("../src/repositories/token_metadata/missing_mints_of_all_pools.sql")
    );
    let plan: Vec<String> = sqlx::query_scalar(explain).fetch_all(&pool).await.unwrap();
    let plan = plan.join("\n");

    assert!(plan.contains("Anti Join"), "{plan}");
    assert!(!plan.contains("SubPlan"), "{plan}");
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
        .list_price_candidates(&PRICE_CANDIDATE_PARAMS)
        .await
        .unwrap()
        .into_iter()
        .map(|candidate| candidate.mint.to_string())
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

// ── The class of a candidate ──────────────────────────────────────────

async fn class_of(pool: &PgPool, mint: &str) -> PriceCandidateClass {
    PgTokenMetadataRepository::new(pool.clone())
        .list_price_candidates(&PRICE_CANDIDATE_PARAMS)
        .await
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.mint.to_string() == mint)
        .expect("a candidate")
        .class
}

async fn known_since(pool: &PgPool, mint: &str, ago: Duration) {
    sqlx::query("UPDATE token_metadata SET fetched_at = $2 WHERE mint = $1")
        .bind(mint)
        .bind(Utc::now() - ago)
        .execute(pool)
        .await
        .unwrap();
}

/// What the side A of pool `addr` holds, in base units of a 9-decimals mint.
async fn holds(pool: &PgPool, addr: &str, reserve_a: i64) {
    sqlx::query("UPDATE pool_current_state SET reserve_a = $2 WHERE pool_address = $1")
        .bind(addr)
        .bind(reserve_a)
        .execute(pool)
        .await
        .unwrap();
}

/// A traded pair from `seed_known_pair_last_moved_at`, its side A priced at 1 $ with `reserve_a`.
async fn traded_mint_holding(pool: &PgPool, reserve_a: i64) -> String {
    let (mint_a, _) = seed_known_pair_last_moved_at(pool, Utc::now()).await;
    price_at(pool, &mint_a, "1", Utc::now()).await;
    holds(pool, &pk(1).to_string(), reserve_a).await;
    mint_a
}

/// Against the grace dropped, or the wrong way round.
#[sqlx::test]
async fn a_mint_without_a_price_is_regular_during_its_grace_then_never_priced(pool: PgPool) {
    let (mint_a, mint_b) = seed_known_pair_last_moved_at(&pool, Utc::now()).await;
    known_since(&pool, &mint_a, Duration::minutes(59)).await;
    known_since(&pool, &mint_b, Duration::minutes(61)).await;

    assert_eq!(class_of(&pool, &mint_a).await, PriceCandidateClass::Regular);
    assert_eq!(
        class_of(&pool, &mint_b).await,
        PriceCandidateClass::NeverPriced
    );
}

/// Against the absence of a price dropped: age alone would make an old mint never priced.
#[sqlx::test]
async fn an_old_mint_with_a_price_is_not_never_priced(pool: PgPool) {
    let mint = traded_mint_holding(&pool, 10_000_000_000).await;
    known_since(&pool, &mint, Duration::days(30)).await;

    assert_eq!(class_of(&pool, &mint).await, PriceCandidateClass::Regular);
}

/// Against `<` turned into `<=`, and the threshold moved.
#[sqlx::test]
async fn ten_dollars_held_is_regular_and_just_under_is_low_value(pool: PgPool) {
    let mint = traded_mint_holding(&pool, 10_000_000_000).await;
    assert_eq!(class_of(&pool, &mint).await, PriceCandidateClass::Regular);

    holds(&pool, &pk(1).to_string(), 9_999_999_999).await;
    assert_eq!(class_of(&pool, &mint).await, PriceCandidateClass::LowValue);
}

/// Against a value read as absent when the reserves are empty.
#[sqlx::test]
async fn a_priced_mint_holding_nothing_is_low_value(pool: PgPool) {
    let mint = traded_mint_holding(&pool, 0).await;

    assert_eq!(class_of(&pool, &mint).await, PriceCandidateClass::LowValue);
}

/// Against the sum left to one pool, or to one side of a pool.
#[sqlx::test]
async fn what_a_mint_holds_adds_up_over_its_pools_and_sides(pool: PgPool) {
    let mint = traded_mint_holding(&pool, 6_000_000_000).await;
    let other = pk(32).to_string();
    let second_pool = pk(2).to_string();
    insert_resolved_pool(&pool, &second_pool, &other, &mint).await;
    insert_pool_state(&pool, &second_pool, Utc::now()).await;
    insert_metadata(&pool, &other).await;
    sqlx::query("UPDATE pool_current_state SET reserve_b = 6000000000 WHERE pool_address = $1")
        .bind(&second_pool)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(class_of(&pool, &mint).await, PriceCandidateClass::Regular);
}

/// Against a missing value read as under the threshold.
#[sqlx::test]
async fn a_priced_mint_without_a_pool_stays_regular(pool: PgPool) {
    let mint = pk(40).to_string();
    insert_metadata(&pool, &mint).await;
    price_at(&pool, &mint, "1", Utc::now()).await;

    assert_eq!(class_of(&pool, &mint).await, PriceCandidateClass::Regular);
}

/// Against the value bounded on the staleness of its price, which would make it absent.
#[sqlx::test]
async fn a_low_value_is_judged_at_the_last_price_whatever_its_age(pool: PgPool) {
    let (mint_a, _) = seed_known_pair_last_moved_at(&pool, Utc::now()).await;
    price_at(&pool, &mint_a, "1", Utc::now() - Duration::days(3)).await;

    assert_eq!(
        class_of(&pool, &mint_a).await,
        PriceCandidateClass::LowValue
    );
}
