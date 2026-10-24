//! `PgMeasureRepository` under `yog_api`, and the signals of one protocol: the produced
//! resources read by scope, from the tables as their writers leave them, seeded here.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::{
    Cursor, PageDirection,
    domain::{
        LiquidityMoveLookup, LiquidityMoveOrder, PoolCreationLookup, PoolDiscoveryLookup, Protocol,
        SignalFeed, SignalFilter, Span, SwapLookup, TvlLookup, Window,
    },
};
use yog_persistence::{PgMeasureRepository, PgSignalRepository};

use super::helpers::{pk, setup_roles_and_connect_as};

const DAMM: &str = "meteora_damm_v2";
const DLMM: &str = "meteora_dlmm";

/// The close of the last frozen hour: a day never starts at midnight.
fn as_of() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 13, 0, 0).unwrap()
}

/// `hours` hours ending at [`as_of`], the facts holding them all.
fn span(hours: i64) -> Span {
    Span {
        from: as_of() - Duration::hours(hours),
        as_of: as_of(),
        covered_since: as_of() - Duration::hours(hours),
    }
}

async fn repository(pool: &PgPool) -> PgMeasureRepository {
    PgMeasureRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

/// One protocol-hour per hour of `span(hours)`: `volume` dollars and one swap each.
async fn seed_protocol_hours(pool: &PgPool, protocol: &str, hours: i32, volume: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_hourly
           (protocol, bucket, volume_usd, fees_usd, lp_fees_usd, protocol_fees_usd,
            referral_fees_usd, swap_count, active_pools, active_pools_valued)
         SELECT $1, h, $4, 0, 0, 0, 0, 1, 1, 1
           FROM generate_series($2 - make_interval(hours => $3), $2 - INTERVAL '1 hour',
                                INTERVAL '1 hour') AS h",
    )
    .bind(protocol)
    .bind(as_of())
    .bind(hours)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

/// One pool-hour per hour of `span(hours)`, `volume` dollars on side A; the last one unvalued.
async fn seed_pool_hours(pool: &PgPool, address: &str, hours: i32, volume: i64) {
    sqlx::query(
        "INSERT INTO analytic_pool_hourly
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd,
            volume_in_b_usd, fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd)
         SELECT $1, h, 'meteora_damm_v2', 2, h < $2 - INTERVAL '1 hour',
                CASE WHEN h < $2 - INTERVAL '1 hour' THEN $4 END,
                CASE WHEN h < $2 - INTERVAL '1 hour' THEN 0 END, 0, 0, 0, 0
           FROM generate_series($2 - make_interval(hours => $3), $2 - INTERVAL '1 hour',
                                INTERVAL '1 hour') AS h",
    )
    .bind(address)
    .bind(as_of())
    .bind(hours)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the protocol filter of the totals and the series dropped — DLMM's hours counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_swaps_count_only_it(pool: PgPool) {
    seed_protocol_hours(&pool, DAMM, 24, 1).await;
    seed_protocol_hours(&pool, DLMM, 24, 100).await;
    let repository = repository(&pool).await;
    let day = span(24);

    let all = repository
        .protocol_totals(&day, Window::Day, None)
        .await
        .unwrap();
    let damm = repository
        .protocol_totals(&day, Window::Day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let hourly = repository
        .protocol_hourly(&day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let daily = repository
        .protocol_daily(&day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert_eq!(all.len(), 2);
    assert_eq!(damm.len(), 1);
    assert_eq!(damm[0].sums.volume_usd, Some(Decimal::from(24)));
    assert_eq!(damm[0].sums.swaps, 24);
    assert_eq!(hourly.len(), 24);
    assert!(hourly.iter().all(|b| b.protocol == Protocol::MeteoraDammV2));
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0].value.volume_usd, Some(Decimal::from(24)));
}

/// Mutation: the pool filter dropped — the other pool's volume counted; or `valued` ignored in
/// the coverage — the unvalued hour counted as valued.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_swaps_count_only_it_and_say_their_coverage(pool: PgPool) {
    let (mine, other) = (pk(1), pk(2));
    seed_pool_hours(&pool, &mine.to_string(), 24, 10).await;
    seed_pool_hours(&pool, &other.to_string(), 24, 1000).await;
    let repository = repository(&pool).await;
    let day = span(24);

    let totals = repository.pool_totals(&day, mine).await.unwrap().unwrap();
    let hourly = repository.pool_hourly(&day, mine).await.unwrap();

    assert_eq!(totals.volume_usd, Some(Decimal::from(230)));
    assert_eq!(totals.swaps, 48);
    assert_eq!(totals.pool_hours, 24);
    assert_eq!(totals.pool_hours_valued, 23);
    assert_eq!(hourly.len(), 24);
    assert_eq!(hourly.last().unwrap().sums.pool_hours_valued, 0);
    assert!(repository.pool_totals(&day, pk(3)).await.unwrap().is_none());
}

/// Mutation: the pool's days cut at midnight, or the `covered_since` bound dropped — a first
/// day the facts do not hold whole is drawn.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_day_is_24_hours_ending_at_as_of_and_held_whole(pool: PgPool) {
    let mine = pk(1);
    seed_pool_hours(&pool, &mine.to_string(), 24 * 2 + 12, 1).await;
    let repository = repository(&pool).await;
    let three_days = Span {
        covered_since: as_of() - Duration::hours(24 * 2 + 12),
        ..span(24 * 3)
    };

    let daily = repository.pool_daily(&three_days, mine).await.unwrap();

    assert_eq!(daily.len(), 2);
    assert_eq!(daily[0].from, as_of() - Duration::hours(48));
    assert_eq!(daily[1].sums.pool_hours, 24);
}

/// Mutation: the protocol filter of the creations dropped — DLMM's counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_creations_count_only_it(pool: PgPool) {
    for (protocol, births) in [(DAMM, 2), (DLMM, 50)] {
        sqlx::query(
            "INSERT INTO analytic_protocol_births_hourly (protocol, bucket, births)
             VALUES ($1, $2 - INTERVAL '1 hour', $3), ($1, $2 - INTERVAL '2 hours', $3)",
        )
        .bind(protocol)
        .bind(as_of())
        .bind(births)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;
    let day = span(24);

    let totals = PoolCreationLookup::totals(&repository, &day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let hourly = PoolCreationLookup::hourly(&repository, &day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let all = PoolCreationLookup::totals(&repository, &day, None)
        .await
        .unwrap();

    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].value.pools, 4);
    assert_eq!(hourly.len(), 2);
    assert_eq!(all.len(), 2);
}

async fn insert_liquidity_hour(pool: &PgPool, protocol: &str, bucket: DateTime<Utc>, added: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_liquidity_hourly
           (protocol, bucket, created_usd, added_usd, removed_usd, moves, moves_valued)
         VALUES ($1, $2, 0, $3, 0, 1, 1)",
    )
    .bind(protocol)
    .bind(bucket)
    .bind(added)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: `measured_since` read over every protocol, or the protocol filter of the sums
/// dropped — DLMM's earlier start and its liquidity counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_liquidity_starts_and_sums_on_its_own(pool: PgPool) {
    let start = as_of() - Duration::hours(10);
    for h in 0..10 {
        insert_liquidity_hour(&pool, DAMM, start + Duration::hours(h), 5).await;
    }
    insert_liquidity_hour(&pool, DLMM, as_of() - Duration::hours(20), 1000).await;
    let repository = repository(&pool).await;
    let since = repository
        .measured_since(Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let covered = Span {
        covered_since: start,
        ..span(10)
    };

    let totals = LiquidityMoveLookup::totals(&repository, &covered, Protocol::MeteoraDammV2)
        .await
        .unwrap()
        .unwrap();
    let hourly = LiquidityMoveLookup::hourly(&repository, &covered, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(since, Some(start));
    assert_eq!(totals.added_usd, Decimal::from(50));
    assert_eq!(hourly.len(), 10);
}

/// One valued move of `pool`, worth `usd`, `minutes` before [`as_of`]; `None` leaves it unvalued.
async fn insert_move(
    pool: &PgPool,
    address: &str,
    signature: &str,
    minutes: i64,
    usd: Option<i64>,
) {
    let at = as_of() - Duration::minutes(minutes);
    sqlx::query(
        "INSERT INTO analytic_liquidity_moves
           (signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind,
            amount_usd)
         VALUES ($1, 0, $2, time_bucket('5 minutes', $2), $3, 'meteora_damm_v2', $4, 'add', $5)",
    )
    .bind(signature)
    .bind(at)
    .bind(address)
    .bind(pk(9).to_string())
    .bind(usd)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the cursor's key compared on the amount alone, or without the signature — a
/// tie repeated on the next page or skipped; the pool filter dropped; the unvalued kept.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_moves_page_without_repeat_nor_gap_in_each_order(pool: PgPool) {
    let (mine, other) = (pk(1).to_string(), pk(2).to_string());
    // sig2 and sig3 tie on amount and instant: only the signature orders them.
    for (i, (minutes, usd)) in [(5, 30), (15, 20), (25, 20), (25, 20), (45, 10)]
        .into_iter()
        .enumerate()
    {
        insert_move(&pool, &mine, &format!("sig{i}"), minutes, Some(usd)).await;
    }
    insert_move(&pool, &mine, "unvalued", 1, None).await;
    insert_move(&pool, &other, "other", 2, Some(1_000)).await;
    let repository = repository(&pool).await;
    let day = span(24);

    for order in [LiquidityMoveOrder::Amount, LiquidityMoveOrder::At] {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = repository
                .pool_moves(&day, Window::Day, pk(1), order, cursor.take(), 3)
                .await
                .unwrap();
            seen.extend(page.items.iter().map(|m| m.signature.clone()));
            match page.next_cursor {
                Some(Cursor::LiquidityMove(next)) => cursor = Some(*next),
                Some(other) => panic!("{other:?}"),
                None => break,
            }
        }
        assert_eq!(seen, ["sig0", "sig1", "sig2", "sig3", "sig4"], "{order:?}");
    }
}

async fn insert_tvl(pool: &PgPool, protocol: &str, bucket: DateTime<Utc>, tvl: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl
           (protocol, bucket, tvl_usd, pools_priced, pools_unvalued, measured_at)
         VALUES ($1, $2, $3, 1, 0, $2)",
    )
    .bind(protocol)
    .bind(bucket)
    .bind(tvl)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the latest reading taken after the hour asked, or the protocol filter dropped —
/// the reading of the open hour, or DLMM's, read.
#[sqlx::test(migrations = "./migrations")]
async fn the_tvl_is_the_last_reading_at_or_before_the_hour_with_its_instant(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    insert_tvl(&pool, DAMM, last - Duration::hours(3), 100).await;
    insert_tvl(&pool, DAMM, last - Duration::hours(1), 110).await;
    insert_tvl(&pool, DAMM, last + Duration::hours(1), 999).await;
    insert_tvl(&pool, DLMM, last, 5).await;
    let repository = repository(&pool).await;

    let latest = repository
        .latest(last, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let hourly = TvlLookup::hourly(
        &repository,
        last - Duration::hours(3),
        last,
        Some(Protocol::MeteoraDammV2),
    )
    .await
    .unwrap();

    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].tvl_usd, Some(Decimal::from(110)));
    assert_eq!(latest[0].at, last);
    assert_eq!(hourly.len(), 2);
}

/// Mutation: the window's `period` not filtered, or the reading taken after the hour — the
/// 7-day part, or the open hour's, read.
#[sqlx::test(migrations = "./migrations")]
async fn the_discoveries_are_those_of_the_window_at_the_last_reading(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    for (period, bucket, pools) in [
        ("24h", last - Duration::hours(1), 3),
        ("7d", last, 40),
        ("24h", last + Duration::hours(1), 99),
    ] {
        sqlx::query(
            "INSERT INTO analytic_protocol_tvl_discovered (protocol, period, bucket, tvl_usd, pools)
             VALUES ('meteora_damm_v2', $1, $2, 7, $3)",
        )
        .bind(period)
        .bind(bucket)
        .bind(pools)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;

    let found = PoolDiscoveryLookup::totals(&repository, last, Window::Day, None)
        .await
        .unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].pools, 3);
    assert_eq!(found[0].at, last);
    let week = PoolDiscoveryLookup::totals(&repository, last, Window::Week, None)
        .await
        .unwrap();
    assert_eq!(week[0].pools, 40);
}

/// Mutation: the protocol filter of the feed dropped — DLMM's signal listed.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_signals_are_its_own(pool: PgPool) {
    for protocol in [DAMM, DLMM] {
        sqlx::query(
            "INSERT INTO signals
               (detector, protocol, pool_address, severity, value, threshold, message,
                triggered_at)
             VALUES ('flow_imbalance', $1, $2, 'warning', 1, 1, NULL, $3)",
        )
        .bind(protocol)
        .bind(pk(1).to_string())
        .bind(as_of())
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = PgSignalRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let page = repository
        .list(
            SignalFilter {
                protocol: Some(Protocol::MeteoraDammV2),
                ..SignalFilter::default()
            },
            None,
            PageDirection::Next,
            None,
            50,
        )
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].signal.protocol, Protocol::MeteoraDammV2);
}

/// Mutation: the amount compared exactly, not rounded as the cursor holds it — a `Decimal`
/// keeps 28 digits, and the move past its rounding is repeated or skipped.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_a_move_whose_amount_has_more_digits_than_a_cursor(pool: PgPool) {
    let mine = pk(1).to_string();
    for (signature, minutes, usd) in [
        ("long", 30, "2.0000000000000000000000000000000000001"),
        ("round", 20, "2"),
        ("small", 10, "1"),
    ] {
        sqlx::query(
            "INSERT INTO analytic_liquidity_moves
               (signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind,
                amount_usd)
             VALUES ($1, 0, $2, time_bucket('5 minutes', $2), $3, 'meteora_damm_v2', $4, 'add',
                     $5::NUMERIC)",
        )
        .bind(signature)
        .bind(as_of() - Duration::minutes(minutes))
        .bind(&mine)
        .bind(pk(9).to_string())
        .bind(usd)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .pool_moves(
                &span(24),
                Window::Day,
                pk(1),
                LiquidityMoveOrder::Amount,
                cursor.take(),
                1,
            )
            .await
            .unwrap();
        seen.extend(page.items.iter().map(|m| m.signature.clone()));
        match page.next_cursor {
            Some(Cursor::LiquidityMove(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }

    assert_eq!(seen, ["round", "long", "small"]);
}
