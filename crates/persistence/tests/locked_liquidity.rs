//! The DAMM v2 locked liquidity (057): cp-amm's amounts of L, the vesting schedule, the permanent
//! total, and the reading `yog-analytic` writes with the TVL.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::domain::{
    AnalyticRepository, Protocol, ProtocolTvlLookup, TokenTvlLookup, TokenTvlOrder, TvlMeasure,
    VestingUnlockLookup, VestingUnlockOrder, Window, last_closed_hour,
};
use yog_persistence::{
    PgAnalyticRepository, PgProtocolTvlRepository, PgTokenTvlRepository, PgVestingUnlockRepository,
};

use super::helpers::{pk, price_mint, setup_roles_and_connect_as};

/// A full-range creation: its L, price and range, and the amounts it deposited.
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

/// A row of `analytic_vesting_unlocks`: `(pool, position, instant, liquidity, amount_usd)`.
type Release = (
    String,
    String,
    DateTime<Utc>,
    BigDecimal,
    Option<BigDecimal>,
);

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

/// Before the indexer's first report, the slot is unknown: no slot schedule judged at 0.
/// Mutation: the seeded slot 0 read as a slot.
#[sqlx::test]
async fn a_slot_schedule_before_any_slot_is_reported_is_unknown(pool: PgPool) {
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

    let row = locked_at(&pool, &address, 999).await.expect("listed");
    assert_eq!(row.1, None);
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

/// Two locks of one block, the earlier transaction's second in it: time and index misorder them.
/// Mutation: the total of the last event by time and index, which picks 120 here.
#[sqlx::test]
async fn two_locks_of_one_block_keep_the_greater_total(pool: PgPool) {
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
    for (signature, total, index) in [("later", "150", 0), ("earlier", "120", 1)] {
        sqlx::query(
            "INSERT INTO meteora_damm_v2_permanent_lock_position_events
               (pool_address, signature, position, lock_liquidity_amount,
                total_permanent_locked_liquidity, timestamp, slot, event_index)
             VALUES ($1, $2, 'position', 1, $3, '2026-11-25 12:00:00+00', 7, $4)",
        )
        .bind(&address)
        .bind(signature)
        .bind(dec(total))
        .bind(index)
        .execute(&pool)
        .await
        .unwrap();
    }

    assert_eq!(locked_at(&pool, &address, 0).await.unwrap().0, "150");
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
/// Mutations: an unvalued pool summed, a token without a valued side kept or its unknown lock
/// read as 0, the releases' bounds moved, a slot schedule given an instant.
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
        (None, 1),
        "a token whose only locked pool is unvalued: unknown, not 0"
    );

    let all: Vec<Release> = sqlx::query_as(
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

/// One reading of DAMM v2 at `bucket`, measured at `measured_at`, its locked part `locked` or not.
async fn reading(
    pool: &PgPool,
    bucket: DateTime<Utc>,
    measured_at: DateTime<Utc>,
    locked: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl
           (protocol, bucket, tvl_usd, pools_priced, pools_unvalued, measured_at, locked_usd,
            permanent_usd, vesting_usd, pools_locked, pools_locked_unvalued, vesting_covered_since)
         VALUES ('meteora_damm_v2', $1, 1000, 3, 0, $2, $3::NUMERIC, $3::NUMERIC, 0,
                 CASE WHEN $3 IS NULL THEN NULL ELSE 2 END, CASE WHEN $3 IS NULL THEN NULL ELSE 1 END,
                 CASE WHEN $3 IS NULL THEN NULL ELSE '2026-10-03 11:28:37+00'::TIMESTAMPTZ END)",
    )
    .bind(bucket)
    .bind(measured_at)
    .bind(locked)
    .execute(pool)
    .await
    .unwrap();
}

fn hour(h: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_764_000_000 + h * 3_600, 0).unwrap()
}

/// Mutation: the block built from a reading before the measure, or its columns swapped.
#[sqlx::test]
async fn a_reading_carries_its_locked_block_once_measured(pool: PgPool) {
    reading(&pool, hour(0), hour(1), None).await;
    reading(&pool, hour(1), hour(2), Some("250")).await;
    let repo = PgProtocolTvlRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let readings = repo.list_hourly(hour(0), hour(1), None).await.unwrap();

    assert_eq!(readings[0].meteora_damm_v2, None);
    let block = readings[1].meteora_damm_v2.expect("measured");
    assert_eq!(block.locked_usd.to_string(), "250");
    assert_eq!((block.pools_locked, block.pools_locked_unvalued), (2, 1));
    assert!(block.vesting_covered_since.is_some());
}

/// Mutation: either order's key or its tie-break, an unknown locked part sorted first, or the
/// total summed from the rows.
#[sqlx::test]
async fn the_tokens_rank_by_tvl_or_locked_and_page_without_a_gap(pool: PgPool) {
    reading(&pool, hour(0), hour(1), Some("70")).await;
    for (seed, tvl, locked) in [
        (1u8, "500", Some("50")),
        (2, "300", None),
        (3, "300", Some("20")),
        (5, "300", None),
        (4, "100", Some("60")),
    ] {
        sqlx::query(
            "INSERT INTO analytic_tvl_by_token (protocol, bucket, mint, tvl_usd, locked_usd, pools)
             VALUES ('meteora_damm_v2', $1, $2, $3::NUMERIC, $4::NUMERIC, 1)",
        )
        .bind(hour(0))
        .bind(pk(seed).to_string())
        .bind(tvl)
        .bind(locked)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repo = PgTokenTvlRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let mint = |seed: u8| pk(seed);
    // Ties ranked by mint: three at 300 for the TVL, two unknown locked parts.
    let by_mint = |mut seeds: Vec<u8>| {
        seeds.sort_by_key(|seed| mint(*seed).to_string());
        seeds
    };
    let tvl_order = [vec![1], by_mint(vec![2, 3, 5]), vec![4]].concat();
    let locked_order = [vec![4, 1, 3], by_mint(vec![2, 5])].concat();

    for (order, expected) in [
        (TokenTvlOrder::Tvl, tvl_order),
        (TokenTvlOrder::Locked, locked_order),
    ] {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let read = repo
                .list(Protocol::MeteoraDammV2, order, cursor, 1)
                .await
                .unwrap()
                .expect("a reading");
            assert_eq!(read.at, hour(1));
            assert_eq!(
                read.total.locked_usd.map(|v| v.to_string()),
                Some("70".to_string())
            );
            seen.extend(read.page.items.iter().map(|t| t.mint));
            match read.page.next_cursor {
                Some(yog_core::Cursor::TokenTvl(c)) => cursor = Some(*c),
                _ => break,
            }
        }
        let expected: Vec<_> = expected.into_iter().map(mint).collect();
        assert_eq!(seen, expected, "{order:?}");
    }
}

/// Mutation: the window's bound, its start at the reading, either order or its tie-break, or an
/// unvalued release sorted first.
#[sqlx::test]
async fn the_releases_are_those_of_the_window_after_the_reading(pool: PgPool) {
    let at = hour(1) + Duration::minutes(5);
    reading(&pool, hour(0), at, Some("1")).await;
    for (position, ahead, amount) in [
        (1u8, Duration::hours(1), Some("5")),
        (2, Duration::days(2), None),
        (5, Duration::days(1), None),
        (3, Duration::days(3), Some("9")),
        (4, Duration::days(8), Some("100")),
    ] {
        sqlx::query(
            "INSERT INTO analytic_vesting_unlocks
               (protocol, bucket, pool_address, position, unlocks_at, liquidity, amount_usd)
             VALUES ('meteora_damm_v2', $1, $2, $3, $4, 7, $5::NUMERIC)",
        )
        .bind(hour(0))
        .bind(pk(9).to_string())
        .bind(pk(position).to_string())
        .bind(at + ahead)
        .bind(amount)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repo = PgVestingUnlockRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    for (order, expected) in [
        (VestingUnlockOrder::Time, vec![1, 5, 2, 3]),
        (VestingUnlockOrder::Amount, vec![3, 1, 5, 2]),
    ] {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let read = repo
                .list(Protocol::MeteoraDammV2, Window::Week, order, cursor, 1)
                .await
                .unwrap()
                .expect("a reading");
            assert_eq!(read.as_of, at);
            seen.extend(read.page.items.iter().map(|u| u.position));
            match read.page.next_cursor {
                Some(yog_core::Cursor::VestingUnlock(c)) => cursor = Some(*c),
                _ => break,
            }
        }
        let expected: Vec<_> = expected.into_iter().map(pk).collect();
        assert_eq!(seen, expected, "{order:?}");
    }
}

/// Mutation: the releases not summed by position and instant, or the first vesting not written.
#[sqlx::test]
async fn one_position_releasing_twice_at_once_is_one_row(pool: PgPool) {
    let now = now();
    let full = locked_pool(
        &pool,
        1,
        (FULL_RANGE.1, FULL_RANGE.2, FULL_RANGE.3),
        1,
        Some(1),
        "0",
        FULL_RANGE_DEPOSIT,
    )
    .await;
    let cliff = now.timestamp() + 3_600;
    vesting(&pool, &full, 0, (cliff, 0, 10, 0, 0)).await;
    sqlx::query(
        "INSERT INTO meteora_damm_v2_lock_position_events
           (pool_address, signature, position, owner, vesting, cliff_point, period_frequency,
            cliff_unlock_liquidity, liquidity_per_period, number_of_period, timestamp, slot,
            event_index)
         VALUES ($1, 'second', 'position-0', 'owner', 'other-vesting', $2, 0, 15, 0, 0,
                 '2026-10-01 00:00:00+00', 1, 9)",
    )
    .bind(&full)
    .bind(cliff)
    .execute(&pool)
    .await
    .unwrap();
    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);

    assert_eq!(
        repository
            .measure_tvl(last_closed_hour(now), now)
            .await
            .unwrap(),
        TvlMeasure::Measured
    );

    let rows: Vec<(String, BigDecimal)> =
        sqlx::query_as("SELECT position, liquidity FROM analytic_vesting_unlocks")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows, vec![("position-0".to_string(), dec("25"))]);
    let since: (Option<DateTime<Utc>>,) =
        sqlx::query_as("SELECT vesting_covered_since FROM analytic_protocol_tvl")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        since.0.map(|t| t.to_rfc3339()),
        Some("2026-10-01T00:00:00+00:00".to_string())
    );
}
