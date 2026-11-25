//! The DAMM v2 locked liquidity (057): cp-amm's amounts of L, the vesting schedule, the permanent
//! total, and the reading `yog-analytic` writes with the TVL.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::domain::{AnalyticRepository, TvlMeasure, last_closed_hour};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{pk, price_mint, setup_roles_and_connect_as};

/// A real full-range creation (`EMKc66c2…`): its L, price and range, and the amounts it deposited.
const FULL_RANGE: (&str, &str, &str, &str) = (
    "184467440737525020961601000037588",
    "1844674407370955161600",
    "4295048016",
    "79226673521066979257578248091",
);
const FULL_RANGE_DEPOSIT: (i64, i64) = (99_999_997_672, 1_000_000_000_000_000);

fn dec(v: &str) -> BigDecimal {
    BigDecimal::from_str(v).unwrap()
}

fn now() -> DateTime<Utc> {
    Utc::now()
        .duration_trunc(Duration::microseconds(1))
        .unwrap()
}

/// A DAMM v2 pool whose token B is priced at $1 (9 decimals) and token A unlisted, holding
/// `reserves`, its account read with `range = (sqrt_price, sqrt_min, sqrt_max)`.
async fn locked_pool(
    pool: &PgPool,
    seed: u8,
    range: (&str, &str, &str),
    collect_fee_mode: i16,
    unit: Option<i16>,
    permanent: &str,
    reserves: (i64, i64),
) -> String {
    let address = pk(seed).to_string();
    let (mint_a, mint_b) = (pk(seed + 100).to_string(), pk(seed + 200).to_string());
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals) in [(&mint_a, 6i16), (&mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, now(), now())",
        )
        .bind(mint)
        .bind(decimals)
        .execute(pool)
        .await
        .unwrap();
    }
    price_mint(pool, &mint_b, "1.0").await;
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_slot, last_event_index)
         VALUES ($1, 'meteora_damm_v2', now(), 'liquidity_add', 'sig', $2, $3, 1, 0)",
    )
    .bind(&address)
    .bind(reserves.0)
    .bind(reserves.1)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO meteora_damm_v2_pool_properties
           (pool_address, protocol_fee_percent, referral_fee_percent, has_dynamic_fee,
            sqrt_price, sqrt_min_price, sqrt_max_price, collect_fee_mode,
            permanent_lock_liquidity, pool_activation_type)
         VALUES ($1, 20, 20, false, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&address)
    .bind(dec(range.0))
    .bind(dec(range.1))
    .bind(dec(range.2))
    .bind(collect_fee_mode)
    .bind(dec(permanent))
    .bind(unit)
    .execute(pool)
    .await
    .unwrap();
    address
}

/// `(cliff_point, period_frequency, cliff_unlock_liquidity, liquidity_per_period, number_of_period)`.
type Schedule = (i64, i64, i64, i64, i32);

async fn vesting(
    pool: &PgPool,
    address: &str,
    index: i32,
    (cliff, freq, at_cliff, per, n): Schedule,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_lock_position_events
           (pool_address, signature, position, owner, vesting, cliff_point, period_frequency,
            cliff_unlock_liquidity, liquidity_per_period, number_of_period, timestamp, slot,
            event_index)
         VALUES ($1, 'lock', $2, 'owner', 'vesting', $3, $4, $5, $6, $7, now(), 1, $8)",
    )
    .bind(address)
    .bind(format!("position-{index}"))
    .bind(cliff)
    .bind(freq)
    .bind(BigDecimal::from(at_cliff))
    .bind(BigDecimal::from(per))
    .bind(n)
    .bind(index)
    .execute(pool)
    .await
    .unwrap();
}

async fn permanent_lock(pool: &PgPool, address: &str, index: i32, total: &str) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_permanent_lock_position_events
           (pool_address, signature, position, lock_liquidity_amount,
            total_permanent_locked_liquidity, timestamp, slot, event_index)
         VALUES ($1, 'permanent', 'position', 1, $2, now(), $3, $3)",
    )
    .bind(address)
    .bind(dec(total))
    .bind(index)
    .execute(pool)
    .await
    .unwrap();
}

async fn at_slot(pool: &PgPool, slot: i64) {
    sqlx::query(
        "INSERT INTO network_status (id, slot, rpc_latency_ms) VALUES (1, $1, 0)
         ON CONFLICT (id) DO UPDATE SET slot = EXCLUDED.slot",
    )
    .bind(slot)
    .execute(pool)
    .await
    .unwrap();
}

/// One pool's row of `yog_damm_v2_pool_locked_liquidity` at the Unix second `at`:
/// `(permanent, vesting, valued)`, `None` when the pool holds nothing locked.
async fn locked_at(
    pool: &PgPool,
    address: &str,
    at: i64,
) -> Option<(String, Option<String>, bool)> {
    sqlx::query_as(
        "SELECT permanent_liquidity::TEXT, vesting_liquidity::TEXT,
                a_usd_per_liquidity IS NOT NULL OR b_usd_per_liquidity IS NOT NULL
           FROM yog_damm_v2_pool_locked_liquidity(to_timestamp($2)) WHERE pool_address = $1",
    )
    .bind(address)
    .bind(at)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Real creations, three ranges: the amounts of their L are what they deposited, to one unit.
/// Mutation: either term of either amount altered, or the price left unclamped.
#[sqlx::test]
async fn the_amounts_of_l_are_what_a_real_creation_deposited(pool: PgPool) {
    for (l, price, min, max, a, b) in [
        (
            "139987239701011239011314747542458",
            "2430809909746288954100",
            "18446744073709",
            "18446744073709551616000",
            "50000000000",
            "1000000000000000",
        ),
        (
            "2186578352421100603474143087079",
            "24295373255805",
            "21865783524211",
            "79226673521066979257578248091",
            "89999784296322807",
            "15612",
        ),
        (
            FULL_RANGE.0,
            FULL_RANGE.1,
            FULL_RANGE.2,
            FULL_RANGE.3,
            "99999997672",
            "1000000000000000",
        ),
    ] {
        let (gap_a, gap_b): (BigDecimal, BigDecimal) = sqlx::query_as(
            "SELECT abs(amount_a - $5), abs(amount_b - $6)
               FROM yog_damm_v2_liquidity_amounts($1, $2, $3, $4)",
        )
        .bind(dec(l))
        .bind(dec(price))
        .bind(dec(min))
        .bind(dec(max))
        .bind(dec(a))
        .bind(dec(b))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            gap_a <= dec("1") && gap_b <= dec("1"),
            "{l}: off by ({gap_a}, {gap_b})"
        );
    }
}

/// One schedule step by step: 10 at the cliff, then 5 per 100 s over 4 periods.
/// Mutation: the cliff's part released early, a period counted before it ends, or left uncapped.
#[sqlx::test]
async fn a_vesting_releases_its_cliff_then_each_period(pool: PgPool) {
    let address = locked_pool(
        &pool,
        1,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(1),
        "0",
        (1, 1),
    )
    .await;
    vesting(&pool, &address, 0, (1_000, 100, 10, 5, 4)).await;

    for (at, still_locked) in [
        (999, "30"),
        (1_000, "20"),
        (1_099, "20"),
        (1_150, "15"),
        (1_399, "5"),
    ] {
        let row = locked_at(&pool, &address, at).await.expect("still locked");
        assert_eq!(row.1.as_deref(), Some(still_locked), "at {at}");
    }
    assert_eq!(
        locked_at(&pool, &address, 1_400).await,
        None,
        "all released at the end"
    );
}

/// A slot-counted pool is judged at the last indexed slot, whatever the clock says.
/// Mutation: the unit ignored, every schedule read in seconds.
#[sqlx::test]
async fn a_slot_counted_vesting_is_judged_at_the_last_indexed_slot(pool: PgPool) {
    let address = locked_pool(
        &pool,
        1,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(0),
        "0",
        (1, 1),
    )
    .await;
    vesting(&pool, &address, 0, (1_000, 100, 10, 5, 4)).await;
    at_slot(&pool, 1_150).await;

    let row = locked_at(&pool, &address, 999).await.expect("still locked");
    assert_eq!(row.1.as_deref(), Some("15"));
}

/// Mutation: SUM alone, which skips the schedule of unknown unit and reports the others as all.
#[sqlx::test]
async fn a_vesting_of_unknown_unit_leaves_its_pool_unknown(pool: PgPool) {
    let address = locked_pool(
        &pool,
        1,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        None,
        "0",
        (1, 1),
    )
    .await;
    vesting(&pool, &address, 0, (1_000, 100, 10, 5, 4)).await;

    let row = locked_at(&pool, &address, 999).await.expect("listed");
    assert_eq!(row.1, None, "no unit, no remaining liquidity");
}

/// The permanent total is the greater of the account's and the last lock's.
/// Mutation: either source alone, or the first lock taken for the last.
#[sqlx::test]
async fn the_permanent_total_is_the_greater_of_the_account_and_the_last_lock(pool: PgPool) {
    let range = (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3);
    let stale_account = locked_pool(&pool, 1, range, 1, Some(1), "100", (1, 1)).await;
    permanent_lock(&pool, &stale_account, 1, "120").await;
    permanent_lock(&pool, &stale_account, 2, "150").await;
    let fresh_account = locked_pool(&pool, 2, range, 1, Some(1), "200", (1, 1)).await;
    permanent_lock(&pool, &fresh_account, 3, "150").await;

    assert_eq!(locked_at(&pool, &stale_account, 0).await.unwrap().0, "150");
    assert_eq!(locked_at(&pool, &fresh_account, 0).await.unwrap().0, "200");
}

/// Mutation: the compounding mode valued by the concentrated formula, or a missing range read as 0.
#[sqlx::test]
async fn a_compounding_pool_or_one_without_its_range_is_not_valued(pool: PgPool) {
    let compounding = locked_pool(
        &pool,
        1,
        (
            "1698602818067443507",
            "0",
            "340282366920938463463374607431768211455",
        ),
        2,
        Some(1),
        "100",
        (1, 1),
    )
    .await;
    let concentrated = locked_pool(
        &pool,
        2,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(1),
        "100",
        (1, 1),
    )
    .await;
    let unread = locked_pool(
        &pool,
        3,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(1),
        "100",
        (1, 1),
    )
    .await;
    sqlx::query(
        "UPDATE meteora_damm_v2_pool_properties SET sqrt_min_price = NULL WHERE pool_address = $1",
    )
    .bind(&unread)
    .execute(&pool)
    .await
    .unwrap();

    assert!(!locked_at(&pool, &compounding, 0).await.unwrap().2);
    assert!(locked_at(&pool, &concentrated, 0).await.unwrap().2);
    assert!(!locked_at(&pool, &unread, 0).await.unwrap().2);
}

/// The reading, as `yog_analytic`: half a full-range pool's L locked is half its TVL.
/// Mutations: an unvalued pool summed, a token without a valued side kept, the releases' bounds
/// moved, a slot schedule given an instant.
#[sqlx::test]
async fn the_reading_writes_the_locked_part_the_tokens_and_the_releases(pool: PgPool) {
    let now = now();
    let half = (dec(FULL_RANGE.0) / BigDecimal::from(2))
        .round(0)
        .to_string();
    let full = locked_pool(
        &pool,
        1,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(1),
        &half,
        FULL_RANGE_DEPOSIT,
    )
    .await;
    let cliff = now.timestamp() + 3_600;
    vesting(&pool, &full, 0, (cliff, 86_400, 10, 5, 40)).await;
    // Its cliff already past: its own part is no release to come.
    vesting(&pool, &full, 2, (now.timestamp() - 3_600, 86_400, 7, 3, 40)).await;
    let slots = locked_pool(
        &pool,
        2,
        (
            "1698602818067443507",
            "0",
            "340282366920938463463374607431768211455",
        ),
        2,
        Some(0),
        "100",
        (1, 1_000_000_000),
    )
    .await;
    // Read in seconds, this schedule would fall inside the 30 days.
    vesting(&pool, &slots, 1, (cliff, 86_400, 10, 5, 40)).await;
    at_slot(&pool, 999).await;
    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    let hour = last_closed_hour(now);

    assert_eq!(
        repository.measure_tvl(hour, now).await.unwrap(),
        TvlMeasure::Measured
    );

    let (tvl, locked, permanent, vesting_usd, pools, unvalued): (
        BigDecimal, BigDecimal, BigDecimal, BigDecimal, i64, i64,
    ) = sqlx::query_as(
        "SELECT tvl_usd, locked_usd, permanent_usd, vesting_usd, pools_locked, pools_locked_unvalued
           FROM analytic_protocol_tvl WHERE protocol = 'meteora_damm_v2' AND bucket = $1",
    )
    .bind(hour)
    .fetch_one(&pool)
    .await
    .unwrap();
    // The full-range pool holds $1,000,000 of B; the compounding one $1 of B.
    assert_eq!(tvl, dec("1000001"));
    assert!(
        (permanent.clone() - dec("500000")).abs() < dec("0.01"),
        "permanent {permanent}"
    );
    assert!(
        vesting_usd > dec("0") && vesting_usd < dec("0.000001"),
        "vesting {vesting_usd}"
    );
    assert_eq!(locked, permanent + vesting_usd);
    assert_eq!((pools, unvalued), (2, 1));

    let tokens: Vec<(String, BigDecimal, Option<BigDecimal>, i64)> = sqlx::query_as(
        "SELECT mint, tvl_usd, locked_usd, pools FROM analytic_tvl_by_token ORDER BY tvl_usd DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(tokens.len(), 2, "only the two priced B tokens: {tokens:?}");
    assert_eq!(tokens[0].0, pk(201).to_string());
    assert!((tokens[0].2.clone().unwrap() - dec("500000")).abs() < dec("0.01"));
    assert_eq!(
        (tokens[1].2.clone(), tokens[1].3),
        (Some(dec("0")), 1),
        "a pool counted unvalued adds no locked part"
    );

    let all: Vec<(
        String,
        String,
        DateTime<Utc>,
        BigDecimal,
        Option<BigDecimal>,
    )> = sqlx::query_as(
        "SELECT pool_address, position, unlocks_at, liquidity, amount_usd
           FROM analytic_vesting_unlocks ORDER BY unlocks_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        all.iter().all(|r| r.0 == full && r.4.is_some()),
        "none for a slot schedule"
    );
    let past_cliff: Vec<_> = all.iter().filter(|r| r.1 == "position-2").collect();
    assert_eq!(past_cliff.len(), 30, "30 periods, no cliff part");
    assert!(past_cliff.iter().all(|r| r.3 == dec("3")));
    let releases: Vec<(String, DateTime<Utc>, BigDecimal, Option<BigDecimal>)> = all
        .into_iter()
        .filter(|r| r.1 == "position-0")
        .map(|r| (r.0, r.2, r.3, r.4))
        .collect();
    assert_eq!(releases.len(), 30, "the cliff and 29 days of periods");
    assert_eq!(
        (releases[0].1.timestamp(), releases[0].2.clone()),
        (cliff, dec("10"))
    );
    assert!(
        releases
            .iter()
            .all(|r| r.1 > now && r.1 <= now + Duration::days(30))
    );

    let next = hour + Duration::hours(1);
    assert_eq!(
        repository.measure_tvl(next, now).await.unwrap(),
        TvlMeasure::Measured
    );
    let buckets: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
        "SELECT bucket, COUNT(*) FROM analytic_vesting_unlocks GROUP BY bucket
         UNION ALL SELECT bucket, COUNT(*) FROM analytic_tvl_by_token GROUP BY bucket
         ORDER BY 2 DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        buckets,
        vec![(next, 60), (next, 2)],
        "the next reading replaces the last"
    );
}

/// Mutation: the locked reading written after the TVL's commit, which a failure would split.
#[sqlx::test]
async fn a_failed_locked_reading_writes_no_tvl_either(pool: PgPool) {
    locked_pool(
        &pool,
        1,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(1),
        "100",
        FULL_RANGE_DEPOSIT,
    )
    .await;
    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    sqlx::query("REVOKE INSERT ON analytic_tvl_by_token FROM yog_analytic")
        .execute(&pool)
        .await
        .unwrap();
    let now = now();

    assert!(
        repository
            .measure_tvl(last_closed_hour(now), now)
            .await
            .is_err()
    );

    let readings: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM analytic_protocol_tvl")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(readings.0, 0);
}
