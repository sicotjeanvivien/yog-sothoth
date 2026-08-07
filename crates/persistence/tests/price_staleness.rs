//! Integration tests for migration 005 — a price observation has a validity
//! window, and outside it there is no price.
//!
//! Gated behind `integration-tests`. Three things are asserted here, and the
//! third is the one that keeps the other two honest over time:
//!
//!   1. the **as-of** bound (`yog_price_max_age_asof()`, one hour) — a price
//!      fetched too far before the valued bucket or event no longer values it;
//!   2. the **latest** bound (`yog_price_max_age_latest()`, 15 minutes) — a
//!      current-price lookup that only finds a stale observation yields NULL,
//!      which is what makes a `yog-context` outage visible instead of publishing
//!      yesterday's TVL as today's;
//!   3. **every view that reads `token_prices` applies one of the two.** The
//!      policy previously existed at one site out of seventeen; a test that
//!      enumerates the views from the catalog is the constraint that stops the
//!      eighteenth from forgetting it. The two assertions above cover one view
//!      each — this one covers the ones nobody thought to write a test for.
//!
//! Both bounded lookups are exercised through the VIEW rather than through a
//! repository: the repositories `COALESCE` their sums, so a NULL would arrive
//! indistinguishable from a real zero. That collapse is `.project` ticket 08's
//! subject; here the point is what the view itself publishes.

use super::helpers::pk;
use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;

/// The pool and its two tokens: A with 6 decimals, B with 9.
async fn seed_pool(pool: &PgPool) -> (String, String, String) {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&pool_addr)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(pool)
    .await
    .unwrap();

    // Metadata carries no staleness policy — it is a scale, not a quote.
    for (mint, decimals) in [(&mint_a, 6i16), (&mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1,$2,$3,$3)",
        )
        .bind(mint)
        .bind(decimals)
        .bind(Utc::now() - Duration::hours(48))
        .execute(pool)
        .await
        .unwrap();
    }

    (pool_addr, mint_a, mint_b)
}

async fn insert_price(pool: &PgPool, mint: &str, price: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ($1,$2::NUMERIC,'jupiter',$3)",
    )
    .bind(mint)
    .bind(price)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_swap(pool: &PgPool, pool_addr: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1,'sig_stale','a_to_b',1000000,0,0,0,0,0,0,0,0,false,$2,0,0)",
    )
    .bind(pool_addr)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// `volume_a_to_b_usd` as view 023 publishes it for the pool's only bucket.
async fn flow_a_to_b(pool: &PgPool) -> Option<f64> {
    sqlx::query_scalar(
        "SELECT volume_a_to_b_usd::DOUBLE PRECISION
         FROM meteora_damm_v2_pool_hourly_flow",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The start of a settled bucket, three hours back — far enough that `now`
/// cannot drift into it while the test runs.
fn settled_bucket() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

// ── 1. The as-of bound, on the detector path (view 023) ─────────────────────

#[sqlx::test]
async fn as_of_price_inside_the_window_values_the_bucket(pool: PgPool) {
    let (pool_addr, mint_a, _) = seed_pool(&pool).await;
    let bucket = settled_bucket();

    // Ten minutes before the bucket opened: comfortably inside the hour the
    // policy allows.
    insert_price(&pool, &mint_a, "2.0", bucket - Duration::minutes(10)).await;
    insert_swap(&pool, &pool_addr, bucket + Duration::minutes(30)).await;

    let value = flow_a_to_b(&pool)
        .await
        .expect("a price fetched inside the window must value the bucket");
    assert!(
        (value - 2.0).abs() < 1e-6,
        "1.0 token A at $2 is $2 of a_to_b flow, got {value}"
    );
}

#[sqlx::test]
async fn as_of_price_older_than_the_window_does_not_value_the_bucket(pool: PgPool) {
    let (pool_addr, mint_a, _) = seed_pool(&pool).await;
    let bucket = settled_bucket();

    // Ninety minutes before the bucket opened. The only difference with the test
    // above — and it must be the difference between a figure and no figure.
    insert_price(&pool, &mint_a, "2.0", bucket - Duration::minutes(90)).await;
    insert_swap(&pool, &pool_addr, bucket + Duration::minutes(30)).await;

    assert!(
        flow_a_to_b(&pool).await.is_none(),
        "a price older than yog_price_max_age_asof() is not a price for this \
         bucket; publishing $2 here would be the stale valuation the policy exists \
         to remove"
    );
}

// ── 2. The as-of bound, on the API path (view 021) ──────────────────────────

#[sqlx::test]
async fn liquidity_event_value_follows_the_same_as_of_bound(pool: PgPool) {
    let (pool_addr, mint_a, mint_b) = seed_pool(&pool).await;
    let event_at = Utc::now() - Duration::hours(2);

    // The API feed is bound by the same rule as the detectors: both legs priced,
    // but too long before the event to describe it.
    insert_price(&pool, &mint_a, "2.0", event_at - Duration::minutes(90)).await;
    insert_price(&pool, &mint_b, "100.0", event_at - Duration::minutes(90)).await;

    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
            liquidity_delta, reserve_a_after, reserve_b_after, position, owner,
            timestamp, slot, event_index)
         VALUES ($1,'evt_stale','add',1000000,1000000000,0::NUMERIC,0,0,'pos','own',$2,0,0)",
    )
    .bind(&pool_addr)
    .bind(event_at)
    .execute(&pool)
    .await
    .unwrap();

    let value: Option<f64> = sqlx::query_scalar(
        "SELECT value_usd::DOUBLE PRECISION
         FROM meteora_damm_v2_liquidity_events_valued
         WHERE signature = 'evt_stale'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        value.is_none(),
        "the event row must still appear — only its value goes absent — and the \
         frontend renders a dash rather than a stale $102"
    );
}

// ── 3. The latest bound (view 020), the outage this migration makes visible ──

async fn seed_current_state(pool: &PgPool, pool_addr: &str) {
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_slot, last_event_index)
         VALUES ($1,'meteora_damm_v2',$2,'liquidity_add','sig',10000000,1000000000,1,0)",
    )
    .bind(pool_addr)
    .bind(Utc::now())
    .execute(pool)
    .await
    .unwrap();
}

async fn current_tvl(pool: &PgPool) -> Option<f64> {
    sqlx::query_scalar("SELECT tvl_usd::DOUBLE PRECISION FROM pool_current_tvl")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn current_tvl_uses_a_fresh_price(pool: PgPool) {
    let (pool_addr, mint_a, mint_b) = seed_pool(&pool).await;
    seed_current_state(&pool, &pool_addr).await;

    let now = Utc::now();
    insert_price(&pool, &mint_a, "2.0", now - Duration::minutes(5)).await;
    insert_price(&pool, &mint_b, "100.0", now - Duration::minutes(5)).await;

    // 10.0 A × $2 + 1.0 B × $100 = $120.
    let tvl = current_tvl(&pool)
        .await
        .expect("a fresh price values the TVL");
    assert!((tvl - 120.0).abs() < 1e-6, "expected $120, got {tvl}");
}

#[sqlx::test]
async fn current_tvl_goes_absent_when_the_price_stops_being_refreshed(pool: PgPool) {
    let (pool_addr, mint_a, mint_b) = seed_pool(&pool).await;
    seed_current_state(&pool, &pool_addr).await;

    // yog-context died 30 minutes ago. Reserves are current; the prices are not.
    let now = Utc::now();
    insert_price(&pool, &mint_a, "2.0", now - Duration::minutes(30)).await;
    insert_price(&pool, &mint_b, "100.0", now - Duration::minutes(30)).await;

    assert!(
        current_tvl(&pool).await.is_none(),
        "a TVL computed from half-hour-old quotes is not a current TVL; before \
         this policy the dashboard and tvl_drain both kept using it, silently"
    );
}

// ── 4. The constraint: no view reads prices without a bound ─────────────────

#[sqlx::test]
async fn every_view_reading_token_prices_bounds_the_price_age(pool: PgPool) {
    // `pool_price_snapshot` is the one deliberate exemption: it publishes raw
    // inputs WITH their `fetched_at` so that price_oracle_deviation can gate in
    // Rust, paired with its `max_spot_age` guard on `last_swap_at`. The policy
    // binds valuation, not comparison — see migration 005's header.
    const EXEMPT: &str = "pool_price_snapshot";

    let views: Vec<(String, String)> = sqlx::query_as(
        "SELECT viewname, definition FROM pg_views
         WHERE schemaname = 'public' AND definition LIKE '%token_prices%'
         ORDER BY viewname",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(
        views.len() >= 7,
        "expected the seven known price-reading views, found {} — if a view was \
         removed, update this test deliberately rather than letting the guard \
         quietly cover less",
        views.len()
    );

    let exempt_present = views.iter().any(|(name, _)| name == EXEMPT);
    assert!(
        exempt_present,
        "{EXEMPT} no longer reads token_prices — the exemption above is now \
         meaningless and must be removed"
    );

    // ⚠️ Counted per LOOKUP, not per view. `meteora_damm_v2_pool_hourly_activity`
    // holds five independent price LATERALs (`liq_v` ×2, `pos_fee_v` ×2,
    // `reward_v` ×1); a `contains("yog_price_max_age")` would stay green with
    // four of the five unbounded — reproducing, inside the very guard against it,
    // the "rule applied at one site out of seventeen" this policy exists to fix.
    //
    // The invariant is exact: every bounded view has one bound per lookup
    // (2/2 five times over, 5/5 for the activity view).
    let unbounded: Vec<String> = views
        .iter()
        .filter(|(name, _)| name != EXEMPT)
        .filter_map(|(name, def)| {
            let lookups = def.matches("FROM token_prices").count();
            let bounds = def.matches("yog_price_max_age").count();
            (bounds < lookups).then(|| format!("{name} ({bounds}/{lookups} bornés)"))
        })
        .collect();

    assert!(
        unbounded.is_empty(),
        "these views read token_prices without applying the staleness policy to \
         every lookup: {unbounded:?}. Add `fetched_at >= <reference> - \
         yog_price_max_age_asof()` (or the _latest() variant for a current-price \
         lookup) — the rule used to live at one site out of seventeen, which is \
         how it got lost"
    );
}
