//! `reward_v` in `meteora_damm_v2_pool_hourly_activity`: a bucket holds one row per reward
//! mint. One mint that paid out and cannot be valued — no price or no metadata — nulls the
//! bucket; a mint that paid nothing needs no price.

use super::helpers::{claim_reward, pk, price_mint_since};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

/// Two hours back: clear of the current, still-open bucket.
fn event_time() -> DateTime<Utc> {
    Utc::now() - Duration::hours(2)
}

async fn insert_metadata(pool: &PgPool, mint: &str, decimals: i16) {
    let at = Utc::now() - Duration::days(1);
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1,$2,$3,$3)",
    )
    .bind(mint)
    .bind(decimals)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// `None`: no row for this pool; `Some(None)`: the bucket exists, its total unknown.
/// Asserts at most one bucket: `fetch_optional` would read an arbitrary one.
async fn rewards_usd(pool: &PgPool, pool_addr: &str) -> Option<Option<f64>> {
    let rows = sqlx::query_as::<_, (Option<f64>,)>(
        "SELECT rewards_claimed_usd::DOUBLE PRECISION
         FROM meteora_damm_v2_pool_hourly_activity
         WHERE pool_address = $1",
    )
    .bind(pool_addr)
    .fetch_all(pool)
    .await
    .unwrap();

    assert!(
        rows.len() <= 1,
        "the fixture put activity in {} distinct hours; this helper reads a \
         single bucket and would be answering about an arbitrary one",
        rows.len()
    );
    rows.into_iter().next().map(|(v,)| v)
}

#[sqlx::test]
async fn mixing_a_priced_and_an_unpriced_mint_yields_null(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (priced, unpriced) = (pk(2).to_string(), pk(3).to_string());
    let at = event_time();

    insert_metadata(&pool, &priced, 6).await;
    insert_metadata(&pool, &unpriced, 6).await;
    price_mint_since(&pool, &priced, "2.0", 6).await;

    claim_reward(&pool, &pool_addr, "r1", &priced, 0, 1_000_000, at).await;
    claim_reward(&pool, &pool_addr, "r2", &unpriced, 1, 1_000_000, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(None),
        "a bucket holding an unvaluable mint must be NULL — before 010 this \
         returned 2.0, the priced mint's share published as the pair's total"
    );
}

#[sqlx::test]
async fn a_fully_priced_bucket_sums_its_mints(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_x, mint_y) = (pk(2).to_string(), pk(3).to_string());
    let at = event_time();

    insert_metadata(&pool, &mint_x, 6).await;
    insert_metadata(&pool, &mint_y, 6).await;
    price_mint_since(&pool, &mint_x, "2.0", 6).await;
    price_mint_since(&pool, &mint_y, "3.0", 6).await;

    claim_reward(&pool, &pool_addr, "r1", &mint_x, 0, 1_000_000, at).await;
    claim_reward(&pool, &pool_addr, "r2", &mint_y, 1, 1_000_000, at).await;

    // 1 × $2 + 1 × $3. A gate that blocked the complete case too would pass the test above.
    let value = rewards_usd(&pool, &pool_addr)
        .await
        .expect("the bucket exists")
        .expect("both mints are valuable");
    assert!((value - 5.0).abs() < 1e-9, "expected 5.0, got {value}");
}

#[sqlx::test]
async fn a_zero_claim_needs_no_price(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint = pk(2).to_string();
    let at = event_time();

    insert_metadata(&pool, &mint, 6).await;

    claim_reward(&pool, &pool_addr, "r1", &mint, 0, 0, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(Some(0.0)),
        "a claim of zero is worth zero in any currency — the empty leg must not \
         be declared unknown for want of a price"
    );
}

#[sqlx::test]
async fn an_unresolved_reward_mint_yields_null(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (resolved, unresolved) = (pk(2).to_string(), pk(3).to_string());
    let at = event_time();

    // Both priced, one without decimals: the metadata mechanism alone.
    insert_metadata(&pool, &resolved, 6).await;
    price_mint_since(&pool, &resolved, "2.0", 6).await;
    price_mint_since(&pool, &unresolved, "5.0", 6).await;

    claim_reward(&pool, &pool_addr, "r1", &resolved, 0, 1_000_000, at).await;
    claim_reward(&pool, &pool_addr, "r2", &unresolved, 1, 1_000_000, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(None),
        "an unresolved mint is unvaluable like an unpriced one — before 010 the \
         INNER JOIN dropped its row and 2.0 was published as the pair's total"
    );
}

#[sqlx::test]
async fn an_unresolved_reward_mint_still_produces_a_bucket(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint = pk(2).to_string();
    let at = event_time();

    // Mutation: an INNER join on `token_metadata`, which removes the only row and the hour.
    price_mint_since(&pool, &mint, "5.0", 6).await;
    claim_reward(&pool, &pool_addr, "r1", &mint, 0, 1_000_000, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(None),
        "the hour must appear with an unknown total — \"it was claimed, we could \
         not price it\", not \"nothing happened\""
    );
}
