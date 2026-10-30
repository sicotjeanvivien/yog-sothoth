//! `PgRotationRunRepository` under `yog_api`: the runs started in a period through the last
//! tranche written, listed in two orders and summed, and the protocol's volume by frozen hour
//! split by them.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::{
    Cursor,
    domain::{
        FrozenPeriod, Protocol, ProtocolBucket, RotationRunLookup, RotationRunOrder,
        RotationRunScope, RunSplit, RunsFirstHour, TranchePeriod, Window,
    },
};
use yog_persistence::PgRotationRunRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::minutes(minutes)
}

/// Inserts a run of `pool` starting `start` minutes from `at(0)`, and its hours.
async fn insert_run(
    pool: &PgPool,
    protocol: &str,
    address: u8,
    start: i64,
    volume: &str,
    hours: &[(i64, &str)],
) {
    sqlx::query(
        "INSERT INTO analytic_rotation_runs (pool_address, started_at, protocol, ended_at,
           volume_usd, first_hour_volume_usd, extinct, settled)
         VALUES ($1, $2, $3, $2 + INTERVAL '10 minutes', $4::NUMERIC, $4::NUMERIC / 2, false, false)",
    )
    .bind(pk(address).to_string())
    .bind(at(start))
    .bind(protocol)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
    for (hour, hour_volume) in hours {
        sqlx::query(
            "INSERT INTO analytic_rotation_run_hours (pool_address, started_at, bucket, protocol,
               volume_usd)
             VALUES ($1, $2, $3, $4, $5::NUMERIC)",
        )
        .bind(pk(address).to_string())
        .bind(at(start))
        .bind(at(hour * 60))
        .bind(protocol)
        .bind(*hour_volume)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn insert_tranche(pool: &PgPool, minutes: i64, rotations: bool) {
    sqlx::query(
        "INSERT INTO analytic_tranches (bucket, written_at, rotations) VALUES ($1, $1, $2)",
    )
    .bind(at(minutes))
    .bind(rotations)
    .execute(pool)
    .await
    .unwrap();
}

fn period(from: i64, as_of: i64) -> TranchePeriod {
    TranchePeriod {
        from: at(from),
        as_of: at(as_of),
        covered_since: at(from),
    }
}

fn damm() -> RotationRunScope {
    RotationRunScope::Protocol(Protocol::MeteoraDammV2)
}

async fn read_all(
    repository: &PgRotationRunRepository,
    period: &TranchePeriod,
    scope: RotationRunScope,
    order: RotationRunOrder,
    limit: i64,
) -> Vec<(String, DateTime<Utc>, Decimal)> {
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_runs(period, Window::Day, scope, order, cursor.take(), limit)
            .await
            .unwrap();
        seen.extend(
            page.items
                .iter()
                .map(|r| (r.pool_address.to_string(), r.started_at, r.volume_usd)),
        );
        // ⚠️ A cursor that rereads its own row would page forever: fail instead of hanging.
        assert!(seen.len() <= 100, "paging does not end: {seen:?}");
        match page.next_cursor {
            Some(Cursor::RotationRun(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    seen
}

/// Mutation: a pool's list ranked backwards before its page is cut — its smallest run or its oldest
/// listed first; or another pool's runs read.
#[sqlx::test(migrations = "./migrations")]
async fn the_runs_of_a_pool_page_in_each_order(pool: PgPool) {
    insert_run(&pool, "meteora_damm_v2", 1, -50, "5", &[]).await;
    insert_run(&pool, "meteora_damm_v2", 1, -40, "1", &[]).await;
    insert_run(&pool, "meteora_damm_v2", 1, -30, "3", &[]).await;
    insert_run(&pool, "meteora_damm_v2", 2, -20, "9", &[]).await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let scope = RotationRunScope::Pool(pk(1));

    let by_volume = read_all(
        &repository,
        &period(-60, 0),
        scope,
        RotationRunOrder::Volume,
        1,
    )
    .await;
    let by_date = read_all(&repository, &period(-60, 0), scope, RotationRunOrder::At, 1).await;

    assert_eq!(
        by_volume.iter().map(|r| r.1).collect::<Vec<_>>(),
        [at(-50), at(-30), at(-40)]
    );
    assert_eq!(
        by_date.iter().map(|r| r.1).collect::<Vec<_>>(),
        [at(-30), at(-40), at(-50)]
    );
}

/// Mutation: the period's start or end bound dropped or moved, the protocol or pool filter
/// dropped, or a first hour summed before it ends inside the period — a run listed out of it,
/// another scope's, or one with no time past its first hour counting all its volume in it.
#[sqlx::test(migrations = "./migrations")]
async fn a_list_and_its_totals_read_the_runs_started_in_their_period(pool: PgPool) {
    insert_run(&pool, "meteora_damm_v2", 1, -61, "1", &[]).await;
    insert_run(&pool, "meteora_damm_v2", 1, -60, "2", &[]).await;
    insert_run(&pool, "meteora_damm_v2", 2, -30, "3", &[]).await;
    insert_run(&pool, "meteora_damm_v2", 2, 0, "4", &[]).await;
    insert_run(&pool, "meteora_dlmm", 3, -30, "5", &[]).await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let last_hour = period(-60, 0);

    let all = read_all(&repository, &last_hour, damm(), RotationRunOrder::At, 50).await;
    let one_pool = read_all(
        &repository,
        &last_hour,
        RotationRunScope::Pool(pk(2)),
        RotationRunOrder::At,
        50,
    )
    .await;
    let totals = repository
        .find_totals(&last_hour, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let two_hours = repository
        .find_totals(&period(-120, 0), Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(
        all.iter().map(|r| r.1).collect::<Vec<_>>(),
        [at(-30), at(-60)]
    );
    assert_eq!(one_pool.iter().map(|r| r.1).collect::<Vec<_>>(), [at(-30)]);
    assert_eq!(
        (totals.runs, totals.volume_usd, totals.first_hour),
        (2, Decimal::from(5), None)
    );
    assert_eq!(
        (two_hours.runs, two_hours.first_hour),
        (
            3,
            Some(RunsFirstHour {
                runs: 1,
                volume_usd: Decimal::from(1),
                in_first_hour_usd: Decimal::new(5, 1),
            })
        )
    );
}

/// Mutation: a tie-break dropped or reversed, or the cursor compared with `<=` — a run skipped,
/// repeated, or listed out of its order.
#[sqlx::test(migrations = "./migrations")]
async fn both_orders_page_without_repeat_nor_gap(pool: PgPool) {
    for (address, start, volume) in [
        (1, -50, "7"),
        (2, -55, "7"),
        (1, -40, "9"),
        (3, -40, "7"),
        (2, -10, "1.0000001"),
        (3, -10, "1.0000004"),
    ] {
        insert_run(&pool, "meteora_damm_v2", address, start, volume, &[]).await;
    }
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let last_hour = period(-60, 0);

    let by_volume = read_all(
        &repository,
        &last_hour,
        damm(),
        RotationRunOrder::Volume,
        50,
    )
    .await;
    let by_volume_one =
        read_all(&repository, &last_hour, damm(), RotationRunOrder::Volume, 1).await;
    let by_at = read_all(&repository, &last_hour, damm(), RotationRunOrder::At, 50).await;
    let by_at_one = read_all(&repository, &last_hour, damm(), RotationRunOrder::At, 1).await;

    let key = |r: &(String, DateTime<Utc>, Decimal)| (r.0.clone(), r.1);
    let (p1, p2, p3) = (pk(1).to_string(), pk(2).to_string(), pk(3).to_string());
    assert_eq!(by_volume_one, by_volume);
    assert_eq!(by_at_one, by_at);
    // The most volume first; ties by pool, then start. The two runs under 2 tie once rounded
    // to the cursor's six places, and break by pool against their unrounded order.
    let (lower, higher) = if p2 < p3 { (&p2, &p3) } else { (&p3, &p2) };
    let mut sevens = vec![
        (p1.clone(), at(-50)),
        (p2.clone(), at(-55)),
        (p3.clone(), at(-40)),
    ];
    sevens.sort();
    let mut expected = vec![(p1.clone(), at(-40))];
    expected.extend(sevens);
    expected.push((lower.clone(), at(-10)));
    expected.push((higher.clone(), at(-10)));
    assert_eq!(by_volume.iter().map(key).collect::<Vec<_>>(), expected);
    assert!(by_at.windows(2).all(|w| w[0].1 >= w[1].1), "{by_at:?}");
    assert_eq!(by_at.len(), 6);
}

/// Inserts `protocol`'s fact of each hour, counted from `at(0)`, with its volume.
async fn insert_protocol_hours(pool: &PgPool, protocol: &str, hours: &[(i64, &str)]) {
    for (hour, volume) in hours {
        sqlx::query(
            "INSERT INTO analytic_protocol_hourly
               (protocol, bucket, volume_usd, swap_count, active_pools, active_pools_valued)
             VALUES ($1, $2, $3::NUMERIC, 1, 1, 1)",
        )
        .bind(protocol)
        .bind(at(hour * 60))
        .bind(*volume)
        .execute(pool)
        .await
        .unwrap();
    }
}

fn splits(bars: &[ProtocolBucket<RunSplit>]) -> Vec<(DateTime<Utc>, Decimal, Option<Decimal>)> {
    bars.iter()
        .map(|b| (b.from, b.value.volume_usd.unwrap(), b.value.in_runs_usd))
        .collect()
}

/// Mutation: the split drawn before the runs' first hour or for a protocol never measured, an hour
/// without a run left unsplit, the coverage or the end dropped, or another protocol's rows read.
#[sqlx::test(migrations = "./migrations")]
async fn the_bars_split_the_protocols_hours_from_the_runs_first_hour(pool: PgPool) {
    insert_protocol_hours(
        &pool,
        "meteora_damm_v2",
        &[
            (-6, "1"),
            (-4, "5"),
            (-3, "10"),
            (-2, "20"),
            (-1, "30"),
            (0, "40"),
        ],
    )
    .await;
    insert_protocol_hours(&pool, "meteora_dlmm", &[(-2, "99")]).await;
    insert_run(
        &pool,
        "meteora_damm_v2",
        1,
        -150,
        "5",
        &[(-2, "2"), (-1, "3")],
    )
    .await;
    insert_run(
        &pool,
        "meteora_damm_v2",
        2,
        -90,
        "15",
        &[(-2, "4"), (-1, "5"), (0, "6")],
    )
    .await;
    insert_run(&pool, "meteora_dlmm", 3, -150, "5", &[(-2, "5")]).await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let frozen = FrozenPeriod {
        from: at(-24 * 60),
        as_of: at(0),
        covered_since: at(-4 * 60),
    };

    let hourly = repository
        .list_hourly(&frozen, Some(at(-3 * 60)), Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let never_measured = repository
        .list_hourly(&frozen, None, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    let dec = |v: i64| Decimal::from(v);
    assert_eq!(
        splits(&hourly),
        [
            (at(-4 * 60), dec(5), None),
            (at(-3 * 60), dec(10), Some(dec(0))),
            (at(-2 * 60), dec(20), Some(dec(6))),
            (at(-60), dec(30), Some(dec(8))),
        ]
    );
    assert!(never_measured.iter().all(|b| b.value.in_runs_usd.is_none()));
    assert_eq!(never_measured.len(), 4);
}

/// Mutation: the runs joined hour by hour unsummed — an hour of two runs counted twice in its day;
/// or a day split though the runs start inside it, or never measured.
#[sqlx::test(migrations = "./migrations")]
async fn a_day_sums_each_hour_once_and_is_split_only_whole(pool: PgPool) {
    insert_protocol_hours(
        &pool,
        "meteora_damm_v2",
        &[(-70, "7"), (-30, "3"), (-5, "9"), (-3, "1")],
    )
    .await;
    insert_run(&pool, "meteora_damm_v2", 1, -5 * 60, "2", &[(-5, "2")]).await;
    insert_run(&pool, "meteora_damm_v2", 2, -5 * 60 + 10, "4", &[(-5, "4")]).await;
    insert_run(&pool, "meteora_damm_v2", 3, -30 * 60, "1", &[(-30, "1")]).await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let frozen = FrozenPeriod {
        from: at(-72 * 60),
        as_of: at(0),
        covered_since: at(-96 * 60),
    };

    let daily = repository
        .list_daily(&frozen, Some(at(-40 * 60)), Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let never_measured = repository
        .list_daily(&frozen, None, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    let dec = |v: i64| Decimal::from(v);
    assert_eq!(
        splits(&daily),
        [
            (at(-72 * 60), dec(7), None),
            (at(-48 * 60), dec(3), None),
            (at(-24 * 60), dec(10), Some(dec(6))),
        ]
    );
    assert!(never_measured.iter().all(|b| b.value.in_runs_usd.is_none()));
    assert_eq!(never_measured.len(), 3);
}

/// Mutation: an hour not valued split anyway — its runs' volume drawn against no volume; or a
/// day holding one split, as its 6-hour slice would not be.
#[sqlx::test(migrations = "./migrations")]
async fn a_bar_with_an_hour_not_valued_is_not_split(pool: PgPool) {
    insert_protocol_hours(&pool, "meteora_damm_v2", &[(-1, "10")]).await;
    sqlx::query(
        "INSERT INTO analytic_protocol_hourly
           (protocol, bucket, volume_usd, swap_count, active_pools, active_pools_valued)
         VALUES ('meteora_damm_v2', $1, NULL, 1, 1, 0)",
    )
    .bind(at(-2 * 60))
    .execute(&pool)
    .await
    .unwrap();
    insert_run(
        &pool,
        "meteora_damm_v2",
        1,
        -150,
        "7",
        &[(-2, "3"), (-1, "4")],
    )
    .await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let frozen = FrozenPeriod {
        from: at(-24 * 60),
        as_of: at(0),
        covered_since: at(-24 * 60),
    };
    let since = Some(at(-24 * 60));

    let hourly = repository
        .list_hourly(&frozen, since, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let daily = repository
        .list_daily(&frozen, since, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    let read = |bars: &[ProtocolBucket<RunSplit>]| {
        bars.iter()
            .map(|b| (b.value.volume_usd, b.value.in_runs_usd))
            .collect::<Vec<_>>()
    };
    let dec = |v: i64| Some(Decimal::from(v));
    assert_eq!(read(&hourly), [(None, None), (dec(10), dec(4))]);
    assert_eq!(read(&daily), [(dec(10), None)]);
}

/// Mutation: the coverage read from another column, or every tranche taken as measured — a
/// protocol covered before its rotation was, or the end of the tranches misread.
#[sqlx::test(migrations = "./migrations")]
async fn the_coverage_is_the_first_tranche_measured_and_the_end_the_last_written(pool: PgPool) {
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let before = (
        repository
            .find_measured_since(Protocol::MeteoraDammV2)
            .await
            .unwrap(),
        repository.find_written_through().await.unwrap(),
    );
    insert_tranche(&pool, -20, false).await;
    insert_tranche(&pool, -15, true).await;
    insert_tranche(&pool, -10, true).await;

    let since = repository
        .find_measured_since(Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let through = repository.find_written_through().await.unwrap();
    let dlmm = repository
        .find_measured_since(Protocol::MeteoraDlmm)
        .await
        .unwrap();

    assert_eq!(before, (None, None));
    assert_eq!(since, Some(at(-15)));
    assert_eq!(through, Some(at(-5)));
    assert_eq!(dlmm, None);
}

/// Inserts a run of `address` lasting `minutes`, extinct or not.
async fn insert_lasting(pool: &PgPool, address: u8, start: i64, minutes: i64, extinct: bool) {
    sqlx::query(
        "INSERT INTO analytic_rotation_runs (pool_address, started_at, protocol, ended_at,
           volume_usd, first_hour_volume_usd, extinct, settled)
         VALUES ($1, $2, 'meteora_damm_v2', $2 + make_interval(mins => $3::INT), 1, 1, $4, false)",
    )
    .bind(pk(address).to_string())
    .bind(at(start))
    .bind(minutes)
    .bind(extinct)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the distinct count dropped, the extinct filter reversed, or a percentile taken at
/// another rank — the pools counted per run, the extinct runs counted in progress, or another
/// duration said.
#[sqlx::test(migrations = "./migrations")]
async fn the_totals_say_their_pools_runs_in_progress_and_durations(pool: PgPool) {
    insert_lasting(&pool, 1, -50, 5, true).await;
    insert_lasting(&pool, 1, -40, 10, true).await;
    insert_lasting(&pool, 2, -30, 20, true).await;
    insert_lasting(&pool, 3, -20, 40, false).await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let totals = repository
        .find_totals(&period(-60, 0), Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let none = repository
        .find_totals(&period(-120, -60), Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(
        (
            totals.runs,
            totals.pools,
            totals.in_progress,
            totals.median_duration_seconds,
            totals.p90_duration_seconds
        ),
        (4, 3, 1, Some(900), Some(2040))
    );
    assert_eq!(
        (
            none.runs,
            none.median_duration_seconds,
            none.p90_duration_seconds
        ),
        (0, None, None)
    );
}

/// Mutation: a token joined by the other mint, or a pool without metadata dropped by an inner
/// join — a run named after the wrong side, or missing from the list.
#[sqlx::test(migrations = "./migrations")]
async fn a_listed_run_carries_its_tokens_symbols(pool: PgPool) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint, fee_bps,
                            first_seen_at, last_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2, $3, 25, $4, $4)",
    )
    .bind(pk(1).to_string())
    .bind(pk(61).to_string())
    .bind(pk(62).to_string())
    .bind(at(-300))
    .execute(&pool)
    .await
    .unwrap();
    for (mint, symbol) in [(61, "SOL"), (62, "ABC")] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, symbol, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, 9, $3, $3)",
        )
        .bind(pk(mint).to_string())
        .bind(symbol)
        .bind(at(-300))
        .execute(&pool)
        .await
        .unwrap();
    }
    insert_lasting(&pool, 1, -30, 5, true).await;
    insert_lasting(&pool, 2, -20, 5, true).await;
    let repository =
        PgRotationRunRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let repository = &repository;
    let named = |scope| async move {
        let mut names = Vec::new();
        for order in [RotationRunOrder::Volume, RotationRunOrder::At] {
            let page = repository
                .list_runs(&period(-60, 0), Window::Day, scope, order, None, 50)
                .await
                .unwrap();
            names.push(
                page.items
                    .iter()
                    .map(|r| (r.token_a_symbol.clone(), r.token_b_symbol.clone()))
                    .collect::<Vec<_>>(),
            );
        }
        names
    };
    let sol_abc = (Some("SOL".to_owned()), Some("ABC".to_owned()));

    assert_eq!(
        named(damm()).await,
        [
            [sol_abc.clone(), (None, None)],
            [(None, None), sol_abc.clone()]
        ]
    );
    assert_eq!(
        named(RotationRunScope::Pool(pk(1))).await,
        [[sol_abc.clone()], [sol_abc]]
    );
    assert_eq!(
        named(RotationRunScope::Pool(pk(2))).await,
        [[(None, None)], [(None, None)]]
    );
}
