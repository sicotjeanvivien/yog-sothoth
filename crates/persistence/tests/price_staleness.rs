//! A price observation has a validity window, and outside it there is no price: the as-of
//! bound (one hour) and the latest bound (15 minutes), asserted through the views
//! themselves. `yog_price_asof` alone reads `token_prices`, and every price reader bounds its age.

use super::helpers::{catalog_definitions, pk, price_at};
use chrono::{DateTime, Duration, DurationRound, Utc};
use rust_decimal::Decimal;
use sqlx::{AssertSqlSafe, PgPool};

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

async fn insert_liquidity_event(pool: &PgPool, pool_addr: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
            liquidity_delta, reserve_a_after, reserve_b_after, position, owner,
            timestamp, slot, event_index)
         VALUES ($1,'evt_stale','add',1000000,1000000000,0::NUMERIC,0,0,'pos','own',$2,0,0)",
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

/// A settled bucket three hours back, which `now` cannot drift into during the test.
fn settled_bucket() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

#[sqlx::test]
async fn as_of_price_inside_the_window_values_the_bucket(pool: PgPool) {
    let (pool_addr, mint_a, _) = seed_pool(&pool).await;
    let bucket = settled_bucket();

    price_at(&pool, &mint_a, "2.0", bucket - Duration::minutes(10)).await;
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
async fn as_of_price_is_the_latest_inside_the_window(pool: PgPool) {
    // Mutation: `DESC` dropped from `yog_price_asof` — the oldest price of the window wins.
    let (pool_addr, mint_a, _) = seed_pool(&pool).await;
    let bucket = settled_bucket();

    price_at(&pool, &mint_a, "1.0", bucket - Duration::minutes(50)).await;
    price_at(&pool, &mint_a, "2.0", bucket - Duration::minutes(5)).await;
    insert_swap(&pool, &pool_addr, bucket + Duration::minutes(30)).await;

    let value = flow_a_to_b(&pool)
        .await
        .expect("two prices inside the window value the bucket");
    assert!(
        (value - 2.0).abs() < 1e-6,
        "the latest price before the bucket ($2) values it, got {value}"
    );
}

#[sqlx::test]
async fn as_of_price_older_than_the_window_does_not_value_the_bucket(pool: PgPool) {
    let (pool_addr, mint_a, _) = seed_pool(&pool).await;
    let bucket = settled_bucket();

    // The only difference with the test above: ninety minutes instead of ten.
    price_at(&pool, &mint_a, "2.0", bucket - Duration::minutes(90)).await;
    insert_swap(&pool, &pool_addr, bucket + Duration::minutes(30)).await;

    assert!(
        flow_a_to_b(&pool).await.is_none(),
        "a price older than yog_price_max_age_asof() is not a price for this \
         bucket; publishing $2 here would be the stale valuation the policy exists \
         to remove"
    );
}

#[sqlx::test]
async fn liquidity_event_value_follows_the_same_as_of_bound(pool: PgPool) {
    let (pool_addr, mint_a, mint_b) = seed_pool(&pool).await;
    let event_at = Utc::now() - Duration::hours(2);

    price_at(&pool, &mint_a, "2.0", event_at - Duration::minutes(90)).await;
    price_at(&pool, &mint_b, "100.0", event_at - Duration::minutes(90)).await;

    insert_liquidity_event(&pool, &pool_addr, event_at).await;

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

    // Positive control: without it, a view returning NULL unconditionally stays green.
    price_at(&pool, &mint_a, "2.0", event_at - Duration::minutes(20)).await;
    price_at(&pool, &mint_b, "100.0", event_at - Duration::minutes(20)).await;

    let value: Option<f64> = sqlx::query_scalar(
        "SELECT value_usd::DOUBLE PRECISION
         FROM meteora_damm_v2_liquidity_events_valued
         WHERE signature = 'evt_stale'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let value = value.expect("an in-window observation values the very same event");
    // 1.0 A at $2 + 1.0 B at $100 = $102.
    assert!(
        (value - 102.0).abs() < 1e-6,
        "expected $102 once a price lands inside the window, got {value}"
    );
}

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
    price_at(&pool, &mint_a, "2.0", now - Duration::minutes(5)).await;
    price_at(&pool, &mint_b, "100.0", now - Duration::minutes(5)).await;

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

    // Prices 30 minutes old, reserves current: yog-context died.
    let now = Utc::now();
    price_at(&pool, &mint_a, "2.0", now - Duration::minutes(30)).await;
    price_at(&pool, &mint_b, "100.0", now - Duration::minutes(30)).await;

    assert!(
        current_tvl(&pool).await.is_none(),
        "a TVL computed from half-hour-old quotes is not a current TVL; before \
         this policy the dashboard and tvl_drain both kept using it, silently"
    );
}

#[sqlx::test]
async fn current_tvl_is_absent_for_a_token_never_priced(pool: PgPool) {
    let (pool_addr, _mint_a, mint_b) = seed_pool(&pool).await;
    seed_current_state(&pool, &pool_addr).await;

    price_at(&pool, &mint_b, "100.0", Utc::now() - Duration::minutes(5)).await;

    assert!(current_tvl(&pool).await.is_none());
}

#[sqlx::test]
async fn a_side_value_goes_absent_on_its_own_stale_price(pool: PgPool) {
    let (pool_addr, mint_a, mint_b) = seed_pool(&pool).await;
    seed_current_state(&pool, &pool_addr).await;

    let now = Utc::now();
    price_at(&pool, &mint_a, "2.0", now - Duration::minutes(5)).await;
    price_at(&pool, &mint_b, "100.0", now - Duration::minutes(30)).await;

    let (value_a_usd, value_b_usd): (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
        "SELECT value_a_usd, value_b_usd FROM pool_current_tvl WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Mutation: the bound dropped from B's price join in the view.
    assert_eq!(value_a_usd, Some(Decimal::from(20)));
    assert_eq!(
        value_b_usd, None,
        "a side priced more than 15 minutes ago must not be valued"
    );
}

/// The one exemption: it publishes raw inputs with their `fetched_at`, gated in Rust by
/// `price_oracle_deviation`. The policy binds valuation, not comparison.
const EXEMPT: &str = "pool_price_snapshot";

/// `Some((bounds, lookups))` when a view reads prices without one bound per lookup — counted
/// per lookup, since one view holds several. ⚠️ `lookups == 0` fails: a plain JOIN on the
/// table has no `FROM token_prices` to count, and 0/0 would wave it through.
fn unbounded_lookups(definition: &str) -> Option<(usize, usize)> {
    let lookups = definition.matches("FROM token_prices").count()
        + definition.matches("JOIN token_last_price").count()
        + definition.matches("FROM token_last_price").count();
    let bounds = definition.matches("yog_price_max_age").count();
    (lookups == 0 || bounds != lookups).then_some((bounds, lookups))
}

/// Every view, aggregate or function in `public` reading `token_prices` or `token_last_price`.
async fn price_readers(pool: &PgPool) -> Vec<(String, String)> {
    catalog_definitions(pool)
        .await
        .into_iter()
        .filter(|(_, def)| def.contains("token_prices") || def.contains("token_last_price"))
        .collect()
}

/// The one reader of `token_prices`: every as-of lookup is a call to it.
const AS_OF: &str = "yog_price_asof";

/// The price readers that break the policy: any reader of `token_prices` but [`AS_OF`], and,
/// [`EXEMPT`] aside, any reader without one bound per lookup.
fn policy_breaches(readers: &[(String, String)]) -> Vec<String> {
    readers
        .iter()
        .filter_map(|(name, def)| {
            if name != AS_OF && def.contains("token_prices") {
                Some(format!(
                    "{name} (reads token_prices instead of calling {AS_OF})"
                ))
            } else if name == EXEMPT {
                None
            } else {
                unbounded_lookups(def).map(|(b, l)| format!("{name} ({b}/{l} bornés)"))
            }
        })
        .collect()
}

#[sqlx::test]
async fn every_price_reader_follows_the_policy(pool: PgPool) {
    let readers = price_readers(&pool).await;

    // A reader that stops reading prices may leave this list; one that still reads them may not.
    for known in [AS_OF, "pool_current_tvl", EXEMPT] {
        assert!(
            readers.iter().any(|(name, _)| name == known),
            "{known} no longer reads a price — if that is deliberate, update this test \
             rather than letting the guard quietly cover less"
        );
    }

    let breaches = policy_breaches(&readers);
    assert!(
        breaches.is_empty(),
        "these price readers break the policy: {breaches:?}. Look a price up as of an \
         instant with `LEFT JOIN LATERAL {AS_OF}(mint, at)`; bound a current-price lookup \
         with yog_price_max_age_latest()"
    );
}

#[sqlx::test]
async fn the_guard_catches_the_lookup_written_out_in_a_view(pool: PgPool) {
    sqlx::query(
        "CREATE VIEW _guard_probe_written_out AS
           SELECT p.pool_address, pa.price_usd
           FROM pools p
           LEFT JOIN LATERAL (
               SELECT price_usd FROM token_prices
               WHERE mint = p.token_a_mint AND fetched_at <= p.first_seen_at
                 AND fetched_at >= p.first_seen_at - yog_price_max_age_asof()
               ORDER BY fetched_at DESC LIMIT 1
           ) pa ON true",
    )
    .execute(&pool)
    .await
    .unwrap();

    let readers = price_readers(&pool).await;
    let (_, definition) = readers
        .iter()
        .find(|(name, _)| name == "_guard_probe_written_out")
        .expect("the probe reads token_prices, so the enumeration must find it");
    assert_eq!(
        unbounded_lookups(definition),
        None,
        "premise of this test: the copy is bounded, so a count of bounds lets it through"
    );
    let breaches = policy_breaches(&readers);
    assert!(
        breaches.contains(
            &"_guard_probe_written_out (reads token_prices instead of calling yog_price_asof)"
                .to_string()
        ),
        "a bounded copy of the lookup must still be reported: {breaches:?}"
    );
}

#[sqlx::test]
async fn the_price_lookup_is_inlined(pool: PgPool) {
    // Mutation: a `SET` clause on `yog_price_asof` — Postgres stops inlining it, and every
    // price of the valued views becomes a per-row call outside their plan.
    // ⚠️ A hypertable without a chunk leaves no scan in the plan, and the control below fails.
    let (pool_addr, mint_a, _) = seed_pool(&pool).await;
    price_at(&pool, &mint_a, "1.0", Utc::now()).await;
    insert_liquidity_event(&pool, &pool_addr, Utc::now()).await;
    for view in [
        "meteora_damm_v2_pool_hourly_activity",
        "meteora_damm_v2_pool_hourly_flow",
        "meteora_damm_v2_pool_hourly_liquidity_flow",
        "meteora_damm_v2_liquidity_events_valued",
        "meteora_damm_v2_swap_events_hourly_priced",
    ] {
        let plan: Vec<String> = sqlx::query_scalar(AssertSqlSafe(format!(
            "EXPLAIN (VERBOSE, COSTS OFF) SELECT * FROM {view}"
        )))
        .fetch_all(&pool)
        .await
        .unwrap();
        let plan = plan.join("\n");

        assert!(
            !plan.contains(AS_OF),
            "{view} calls {AS_OF} instead of inlining it: {plan}"
        );
        assert!(
            plan.contains("token_prices"),
            "{view}: no price lookup in the plan: {plan}"
        );
    }
}

#[sqlx::test]
async fn the_guard_catches_a_price_read_that_is_not_a_lateral(pool: PgPool) {
    // The guard must bite on the likeliest next shape: a plain join, no `FROM token_prices`.
    sqlx::query(
        "CREATE VIEW _guard_probe_plain_join AS
           SELECT p.pool_address, tp.price_usd
           FROM pools p JOIN token_prices tp ON tp.mint = p.token_a_mint::TEXT",
    )
    .execute(&pool)
    .await
    .unwrap();

    let views = price_readers(&pool).await;
    let (_, definition) = views
        .iter()
        .find(|(name, _)| name == "_guard_probe_plain_join")
        .expect("the probe reads token_prices, so the enumeration must find it");

    assert_eq!(
        definition.matches("FROM token_prices").count(),
        0,
        "premise of this test: a plain join deparses WITHOUT `FROM token_prices`. \
         If Postgres ever renders it otherwise, the 0/0 hole this guards against \
         no longer exists and the `lookups == 0` branch can be revisited"
    );
    assert_eq!(
        unbounded_lookups(definition),
        Some((0, 0)),
        "an unbounded price read must be flagged even when no lookup can be \
         counted — a `bounds < lookups` predicate scores this 0/0 and lets it pass"
    );
}
