//! The DAMM v2 LPs' fees split between claimed and compounded: written by yog-analytic from each
//! tranche's raw swaps, summed per hour and protocol, and read by scope under `yog_api`.

use chrono::{DateTime, Duration, DurationRound, Utc};
use rust_decimal::Decimal;
use sqlx::types::BigDecimal;
use sqlx::{AssertSqlSafe, PgPool};
use yog_core::domain::{
    AnalyticProgress, AnalyticRepository, CycleWrite, MeteoraDammV2LpFeeSplit, Protocol,
    SwapActivity, SwapActivityLookup, TranchePeriod, Window, WrittenTranches,
};
use yog_persistence::{PgAnalyticRepository, PgSwapActivityRepository};

use super::helpers::{
    DAMM, DLMM, as_of, period, pk, price_mint, setup_roles_and_connect_as, sqrt_price_x64,
};

type Split = (Option<BigDecimal>, Option<BigDecimal>, Option<BigDecimal>);

fn hour_ago(hours: i64) -> DateTime<Utc> {
    (Utc::now() - Duration::hours(hours))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

/// Every hour from `first` written, the closed ones frozen.
fn cycle_from(now: DateTime<Utc>, first: DateTime<Utc>) -> CycleWrite {
    CycleWrite::plan(
        now,
        AnalyticProgress {
            first_hour: Some(first),
            last_frozen: None,
            last_tranche: None,
        },
    )
}

/// ⚠️ Token A priced, B not: B's fees are then valued from the gross of their direction, a sale's
/// and a buy's apart, which a price per unit of B would let a mutation pass.
async fn setup_pool(pool: &PgPool) -> String {
    let address = pk(1).to_string();
    let (mint_a, mint_b) = (pk(101).to_string(), pk(201).to_string());
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
    price_mint(pool, &mint_a, "2.0").await;
    address
}

/// A swap's fee, in raw units of the token it is paid in.
struct Fee {
    in_a: bool,
    claiming: i64,
    compounding: i64,
    protocol: i64,
    referral: i64,
}

async fn swap(pool: &PgPool, address: &str, tag: &str, at: DateTime<Utc>, buy: bool, fee: Fee) {
    let (direction, a, b) = if buy {
        ("b_to_a", 2_000_000_000i64, 500_000_000i64)
    } else {
        ("a_to_b", 1_000_000_000, 250_000_000)
    };
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1, $2, $3, $4, $5, 0, 0, $6, $7, $8, $9, $10, $11, $12, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(direction)
    .bind(a)
    .bind(b)
    .bind(sqrt_price_x64(a, b))
    .bind(fee.claiming)
    .bind(fee.protocol)
    .bind(fee.compounding)
    .bind(fee.referral)
    .bind(fee.in_a)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// A sale paying in B, a buy paying in A, a buy paying in B: each compounds a quarter of its LPs'
/// fee, so the tranche's compounded fees are a quarter of its LPs' at any price.
async fn three_swaps_compounding_a_quarter(pool: &PgPool, address: &str, at: DateTime<Utc>) {
    let fee = |in_a, claiming: i64, protocol, referral| Fee {
        in_a,
        claiming,
        compounding: claiming / 3,
        protocol,
        referral,
    };
    swap(
        pool,
        address,
        "sale_b",
        at,
        false,
        fee(false, 3_000_000, 800_000, 200_000),
    )
    .await;
    swap(
        pool,
        address,
        "buy_a",
        at,
        true,
        fee(true, 60_000_000, 16_000_000, 4_000_000),
    )
    .await;
    swap(
        pool,
        address,
        "buy_b",
        at,
        true,
        fee(false, 6_000_000, 1_000_000, 500_000),
    )
    .await;
}

async fn write(pool: &PgPool, cycle: &CycleWrite) {
    PgAnalyticRepository::new(setup_roles_and_connect_as(pool, "yog_analytic").await)
        .write_cycle(cycle)
        .await
        .unwrap();
}

/// `lp_fees_usd`, `claiming_fees_usd`, `compounding_fees_usd` of `table`'s rows at `bucket`.
async fn split_of(pool: &PgPool, table: &str, bucket: DateTime<Utc>) -> Split {
    // `AssertSqlSafe`: a table name cannot be bound; only this file's constants are interpolated.
    sqlx::query_as(AssertSqlSafe(format!(
        "SELECT lp_fees_usd, claiming_fees_usd, compounding_fees_usd FROM {table} WHERE bucket = $1"
    )))
    .bind(bucket)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Mutation: a fee side valued at another's price per unit (a buy's B at a sale's), or the
/// claimed part left at the LPs' whole — the split no longer a quarter, or no longer the whole.
#[sqlx::test(migrations = "./migrations")]
async fn a_tranche_splits_its_lps_fees_and_its_hour_and_protocol_keep_the_split(pool: PgPool) {
    let hour = hour_ago(3);
    let tranche = hour + Duration::minutes(20);
    let address = setup_pool(&pool).await;
    three_swaps_compounding_a_quarter(&pool, &address, tranche + Duration::seconds(30)).await;

    write(&pool, &cycle_from(hour + Duration::minutes(80), hour)).await;

    let (lp, claiming, compounding) = split_of(&pool, "analytic_pool_5min", tranche).await;
    let (lp, claiming, compounding) = (lp.unwrap(), claiming.unwrap(), compounding.unwrap());
    assert!(lp > 0, "{lp}");
    assert_eq!(&compounding * BigDecimal::from(4), lp);
    assert_eq!(&claiming + &compounding, lp);
    let tranche_split = (Some(lp), Some(claiming), Some(compounding));
    assert_eq!(
        split_of(&pool, "analytic_pool_hourly", hour).await,
        tranche_split
    );
    assert_eq!(
        split_of(&pool, "analytic_protocol_hourly", hour).await,
        tranche_split
    );
}

/// Mutation: the raw sums left without `COALESCE` — a tranche whose swaps pay no fee in A reads
/// its compounded fees unknown instead of 0.
#[sqlx::test(migrations = "./migrations")]
async fn a_tranche_that_compounds_nothing_reads_zero_not_unknown(pool: PgPool) {
    let hour = hour_ago(3);
    let tranche = hour + Duration::minutes(20);
    let address = setup_pool(&pool).await;
    let fee = Fee {
        in_a: false,
        claiming: 3_000_000,
        compounding: 0,
        protocol: 800_000,
        referral: 200_000,
    };
    swap(
        &pool,
        &address,
        "sale",
        tranche + Duration::seconds(30),
        false,
        fee,
    )
    .await;

    write(&pool, &cycle_from(hour + Duration::minutes(80), hour)).await;

    let (lp, claiming, compounding) = split_of(&pool, "analytic_pool_5min", tranche).await;
    assert_eq!(compounding, Some(BigDecimal::from(0)));
    assert_eq!(claiming, lp);
}

/// Mutation: a `COALESCE` on the raw sums where they are used — a tranche caught up without its
/// raw swaps reads 0 compounded instead of unknown.
#[sqlx::test(migrations = "./migrations")]
async fn a_tranche_without_its_raw_swaps_reads_its_split_unknown(pool: PgPool) {
    let hour = hour_ago(3);
    let tranche = hour + Duration::minutes(20);
    let address = setup_pool(&pool).await;
    three_swaps_compounding_a_quarter(&pool, &address, tranche + Duration::seconds(30)).await;
    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', $1, $2)")
        .bind(hour)
        .bind(hour + Duration::hours(1))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM meteora_damm_v2_swap_events")
        .execute(&pool)
        .await
        .unwrap();

    write(&pool, &cycle_from(hour + Duration::minutes(80), hour)).await;

    let (lp, claiming, compounding) = split_of(&pool, "analytic_pool_5min", tranche).await;
    assert!(lp.is_some());
    assert_eq!((claiming, compounding), (None, None));
}

async fn pool_tranche(
    pool: &PgPool,
    address: &str,
    at: DateTime<Utc>,
    lp: Option<i64>,
    split: Option<(i64, i64)>,
) {
    sqlx::query(
        "INSERT INTO analytic_pool_5min
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd, volume_in_b_usd,
            fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd,
            claiming_fees_usd, compounding_fees_usd)
         VALUES ($1, $2, 'meteora_damm_v2', 1, $3 IS NOT NULL, $3, $4, $3, $3, $4, $4, $5, $6)",
    )
    .bind(address)
    .bind(at)
    .bind(lp.map(BigDecimal::from))
    .bind(lp.map(|_| BigDecimal::from(0)))
    .bind(split.map(|(c, _)| BigDecimal::from(c)))
    .bind(split.map(|(_, c)| BigDecimal::from(c)))
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the hour's split summed without its `COUNT`, or the protocol's without `OR lp IS
/// NULL` — a tranche written before the split skipped, or an unvalued pool sinking the others.
#[sqlx::test(migrations = "./migrations")]
async fn an_hour_with_a_tranche_written_before_the_split_reads_it_unknown(pool: PgPool) {
    let hour = hour_ago(3);
    let mixed = hour - Duration::hours(1);
    let (before, unvalued, measured) = (pk(1).to_string(), pk(2).to_string(), pk(3).to_string());
    pool_tranche(&pool, &before, mixed + Duration::minutes(5), Some(4), None).await;
    pool_tranche(
        &pool,
        &before,
        mixed + Duration::minutes(10),
        Some(4),
        Some((3, 1)),
    )
    .await;
    pool_tranche(&pool, &unvalued, hour + Duration::minutes(5), None, None).await;
    pool_tranche(
        &pool,
        &measured,
        hour + Duration::minutes(5),
        Some(4),
        Some((3, 1)),
    )
    .await;

    write(&pool, &cycle_from(hour + Duration::minutes(80), mixed)).await;

    let (lp, claiming, compounding): Split = sqlx::query_as(
        "SELECT lp_fees_usd, claiming_fees_usd, compounding_fees_usd
           FROM analytic_pool_hourly WHERE pool_address = $1",
    )
    .bind(&before)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (lp, claiming, compounding),
        (Some(BigDecimal::from(8)), None, None)
    );
    let (_, claiming, compounding) = split_of(&pool, "analytic_protocol_hourly", mixed).await;
    assert_eq!((claiming, compounding), (None, None));
    let (_, claiming, compounding) = split_of(&pool, "analytic_protocol_hourly", hour).await;
    assert_eq!(
        (claiming, compounding),
        (Some(BigDecimal::from(3)), Some(BigDecimal::from(1)))
    );
}

/// Mutation: the split of an hour without a pool left unknown — a quiet hour read as not measured.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocol_hour_without_a_swap_splits_nothing(pool: PgPool) {
    let hour = hour_ago(3);
    write(&pool, &cycle_from(hour + Duration::minutes(80), hour)).await;

    let (lp, claiming, compounding) = split_of(&pool, "analytic_protocol_hourly", hour).await;
    let zero = Some(BigDecimal::from(0));
    assert_eq!(
        (lp, claiming, compounding),
        (zero.clone(), zero.clone(), zero)
    );
}

// The reads, from the hours and tranches as yog-analytic leaves them.

async fn api(pool: &PgPool) -> PgSwapActivityRepository {
    PgSwapActivityRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

fn split(claiming: i64, compounding: i64) -> Option<MeteoraDammV2LpFeeSplit> {
    Some(MeteoraDammV2LpFeeSplit {
        claiming_fees_usd: Some(Decimal::from(claiming)),
        compounding_fees_usd: Some(Decimal::from(compounding)),
    })
}

fn unknown() -> Option<MeteoraDammV2LpFeeSplit> {
    Some(MeteoraDammV2LpFeeSplit {
        claiming_fees_usd: None,
        compounding_fees_usd: None,
    })
}

/// Each hour of `period(24)` for `protocol`: LP fees of 4, split 3 + 1; the first hour written
/// before the split, the tenth unvalued.
async fn protocol_hours(pool: &PgPool, protocol: &str) {
    sqlx::query(
        "INSERT INTO analytic_protocol_hourly
           (protocol, bucket, volume_usd, fees_usd, lp_fees_usd, protocol_fees_usd,
            referral_fees_usd, claiming_fees_usd, compounding_fees_usd,
            swap_count, active_pools, active_pools_valued)
         SELECT $1, h, 0, 0, CASE WHEN h = $2 - INTERVAL '15 hours' THEN NULL ELSE 4 END, 0, 0,
                CASE WHEN h > $2 - INTERVAL '24 hours' AND h <> $2 - INTERVAL '15 hours' THEN 3 END,
                CASE WHEN h > $2 - INTERVAL '24 hours' AND h <> $2 - INTERVAL '15 hours' THEN 1 END,
                1, 1, 1
           FROM generate_series($2 - INTERVAL '24 hours', $2 - INTERVAL '1 hour',
                                INTERVAL '1 hour') AS h",
    )
    .bind(protocol)
    .bind(as_of())
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the guard dropped from a protocol read — the hour before the split skipped, or the
/// unvalued hour sinking its bucket; or the block given to DLMM's row.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_split_is_unknown_over_an_hour_before_it_and_only_damm_carries_it(
    pool: PgPool,
) {
    protocol_hours(&pool, DAMM).await;
    protocol_hours(&pool, DLMM).await;
    let repository = api(&pool).await;
    let day = period(24);

    let totals = repository
        .find_protocol_totals(&day, Window::Day, None)
        .await
        .unwrap();
    let damm = totals
        .iter()
        .find(|r| r.protocol == Protocol::MeteoraDammV2)
        .unwrap();
    let dlmm = totals
        .iter()
        .find(|r| r.protocol == Protocol::MeteoraDlmm)
        .unwrap();
    assert_eq!(damm.activity.meteora_damm_v2, unknown());
    assert_eq!(dlmm.activity.meteora_damm_v2, None);

    let hourly = repository
        .list_protocol_hourly(&day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    assert_eq!(hourly[0].value.meteora_damm_v2, unknown());
    assert_eq!(hourly[1].value.meteora_damm_v2, split(3, 1));

    let slices = repository
        .list_protocol_buckets(&day, Duration::hours(6), Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let drawn: Vec<_> = slices
        .iter()
        .map(|b| b.value.meteora_damm_v2.clone())
        .collect();
    assert_eq!(drawn, [unknown(), split(15, 5), split(18, 6), split(18, 6)]);
}

/// The pool, first seen a day before `as_of`, and the frozen hours of `period(24)`.
async fn know_pool(pool: &PgPool, address: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, first_seen_at, last_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2 - INTERVAL '24 hours', $2)",
    )
    .bind(address)
    .bind(as_of())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO analytic_hours (bucket, frozen_at)
         SELECT h, $1 FROM generate_series($1 - INTERVAL '24 hours', $1 - INTERVAL '1 hour',
                                           INTERVAL '1 hour') AS h
         ON CONFLICT DO NOTHING",
    )
    .bind(as_of())
    .execute(pool)
    .await
    .unwrap();
}

async fn pool_hour(
    pool: &PgPool,
    address: &str,
    at: DateTime<Utc>,
    lp: Option<i64>,
    split: Option<(i64, i64)>,
) {
    sqlx::query(
        "INSERT INTO analytic_pool_hourly
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd, volume_in_b_usd,
            fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd,
            claiming_fees_usd, compounding_fees_usd)
         VALUES ($1, $2, 'meteora_damm_v2', 1, $3 IS NOT NULL, $3, $4, $3, $3, $4, $4, $5, $6)",
    )
    .bind(address)
    .bind(at)
    .bind(lp.map(BigDecimal::from))
    .bind(lp.map(|_| BigDecimal::from(0)))
    .bind(split.map(|(c, _)| BigDecimal::from(c)))
    .bind(split.map(|(_, c)| BigDecimal::from(c)))
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the guard dropped from a pool read, or an hour without a fact read unknown — the
/// hour before the split skipped, the unvalued hour sinking its slice, a quiet slice unknown.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_split_is_unknown_over_an_hour_before_it_and_zero_where_it_did_not_swap(
    pool: PgPool,
) {
    let (old, new) = (pk(1), pk(2));
    for address in [old, new] {
        know_pool(&pool, &address.to_string()).await;
    }
    let at = |hours: i64| as_of() - Duration::hours(hours);
    pool_hour(&pool, &old.to_string(), at(23), Some(4), None).await;
    pool_hour(&pool, &old.to_string(), at(10), None, None).await;
    pool_hour(&pool, &old.to_string(), at(9), Some(4), Some((3, 1))).await;
    pool_hour(&pool, &new.to_string(), at(9), Some(4), Some((3, 1))).await;
    let repository = api(&pool).await;
    let day = period(24);

    let totals = repository
        .find_pool_totals(&day, old)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(totals.meteora_damm_v2, unknown());
    let totals = repository
        .find_pool_totals(&day, new)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(totals.meteora_damm_v2, split(3, 1));

    let hourly = repository.list_pool_hourly(&day, old).await.unwrap();
    let drawn: Vec<_> = hourly
        .iter()
        .map(|b| b.activity.meteora_damm_v2.clone())
        .collect();
    assert_eq!(
        (drawn[1].clone(), drawn[15].clone()),
        (unknown(), split(3, 1))
    );
    assert_eq!(
        drawn[0],
        split(0, 0),
        "an hour without a swap splits nothing"
    );

    let slices = repository
        .list_pool_buckets(&day, Duration::hours(6), old)
        .await
        .unwrap();
    let drawn: Vec<_> = slices
        .iter()
        .map(|b| b.activity.meteora_damm_v2.clone())
        .collect();
    assert_eq!(drawn, [unknown(), split(0, 0), split(3, 1), split(0, 0)]);
}

/// The tranches of the three hours before `through`, written.
async fn written_tranches(pool: &PgPool, through: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO analytic_tranches (bucket, written_at)
         SELECT generate_series($1::TIMESTAMPTZ - INTERVAL '3 hours',
                                $1::TIMESTAMPTZ - INTERVAL '5 minutes', INTERVAL '5 minutes'), now()",
    )
    .bind(through)
    .execute(pool)
    .await
    .unwrap();
}

async fn pool_seen(pool: &PgPool, address: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, first_seen_at, last_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2, $2)",
    )
    .bind(address)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the guard dropped from a tranche read, an unvalued pool sinking its tranche, or a
/// tranche without a fact read unknown.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_split_is_unknown_over_a_tranche_before_it(pool: PgPool) {
    let through = as_of() + Duration::minutes(15);
    written_tranches(&pool, through).await;
    let (old, new, unvalued) = (pk(1), pk(2), pk(3));
    for address in [old, new, unvalued] {
        pool_seen(&pool, &address.to_string(), through - Duration::hours(3)).await;
    }
    let at = |minutes: i64| through - Duration::minutes(minutes);
    pool_tranche(&pool, &old.to_string(), at(55), Some(4), None).await;
    pool_tranche(&pool, &old.to_string(), at(30), Some(4), Some((3, 1))).await;
    pool_tranche(&pool, &new.to_string(), at(30), Some(4), Some((3, 1))).await;
    pool_tranche(&pool, &unvalued.to_string(), at(30), None, None).await;
    let repository = api(&pool).await;
    let hour = TranchePeriod::hour_through(
        WrittenTranches {
            first: through - Duration::hours(3),
            through,
        },
        None,
    );

    let totals = repository
        .find_protocol_totals_in_tranches(&hour, None)
        .await
        .unwrap();
    assert_eq!(totals[0].activity.meteora_damm_v2, unknown());
    let bars = repository
        .list_protocol_tranches(&hour, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let drawn: Vec<_> = bars
        .iter()
        .map(|b| b.value.meteora_damm_v2.clone())
        .collect();
    assert_eq!(
        (drawn[1].clone(), drawn[6].clone()),
        (unknown(), split(6, 2))
    );
    assert_eq!(drawn[0], split(0, 0));

    let old_totals = repository
        .find_pool_totals_in_tranches(&hour, old)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_totals.meteora_damm_v2, unknown());
    let new_totals = repository
        .find_pool_totals_in_tranches(&hour, new)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new_totals.meteora_damm_v2, split(3, 1));
    let bars = repository.list_pool_tranches(&hour, old).await.unwrap();
    let drawn: Vec<SwapActivity> = bars.into_iter().map(|b| b.activity).collect();
    assert_eq!(drawn[1].meteora_damm_v2, unknown());
    assert_eq!(drawn[6].meteora_damm_v2, split(3, 1));
    assert_eq!(drawn[0].meteora_damm_v2, split(0, 0));
}
