//! What a cycle of `PgAnalyticRepository` writes for the Overview, under
//! `yog_analytic` itself: the births, and at the freeze, the distinct pools and
//! the tokens of each window. The tranches are seeded already valued.

use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{
    AnalyticRepository, CycleWrite, TokenActivityCursor, TokenActivityFeed, Window,
};
use yog_persistence::{PgAnalyticRepository, PgTokenActivityRepository};

use super::helpers::{as_role, pk};

fn hour(n: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap() + Duration::hours(n)
}

/// Rewrites the hours `first..=last` from their seeded tranches and freezes
/// through `last`. ⚠️ Their tranches recorded too: an hour freezes only once they are.
fn cycle(first: i64, last: i64) -> CycleWrite {
    CycleWrite {
        tranches_from: hour(first),
        tranches_through: hour(last) + Duration::minutes(55),
        from: hour(first),
        through: hour(last),
        freeze_through: Some(hour(last)),
        tvl_hour: None,
        at: hour(last + 2),
    }
}

async fn setup_pool(pool: &PgPool, seed: u8, mint_a: u8, mint_b: u8) -> String {
    let address = pk(seed).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint, first_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2, $3, $4)",
    )
    .bind(&address)
    .bind(pk(mint_a).to_string())
    .bind(pk(mint_b).to_string())
    .bind(hour(0))
    .execute(pool)
    .await
    .unwrap();
    address
}

/// One valued tranche at the start of hour `h`, worth `volume` dollars; `None` is unvalued.
async fn tranche(pool: &PgPool, address: &str, h: i64, volume: Option<i64>) {
    sqlx::query(
        "INSERT INTO analytic_pool_5min
           (pool_address, bucket, protocol, swap_count, valued,
            volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
            protocol_fees_usd, referral_fees_usd)
         VALUES ($1, $2, 'meteora_damm_v2', 1, $3, $4, 0, 0, 0, 0, 0)",
    )
    .bind(address)
    .bind(hour(h))
    .bind(volume.is_some())
    .bind(volume.map(BigDecimal::from))
    .execute(pool)
    .await
    .unwrap();
}

async fn initialize(pool: &PgPool, address: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_initialize_pool_events
           (pool_address, signature, token_a_mint, token_b_mint, creator, payer, alpha_vault,
            sqrt_min_price, sqrt_max_price, sqrt_price, liquidity,
            activation_type, activation_point, collect_fee_mode, pool_type,
            token_a_flag, token_b_flag, token_a_amount, token_b_amount,
            total_amount_a, total_amount_b, pool_fees_raw, timestamp, slot, event_index)
         VALUES ($1, $1, 'a', 'b', 'c', 'p', 'v', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                 '\\x00', $2, 0, 0)",
    )
    .bind(address)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn active_pools(pool: &PgPool, period: &str, at: i64) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT active_pools FROM analytic_protocol_periods
          WHERE protocol = 'meteora_damm_v2' AND period = $1 AND bucket = $2",
    )
    .bind(period)
    .bind(hour(at))
    .fetch_optional(pool)
    .await
    .unwrap()
}

type TokenRow = (String, DateTime<Utc>, BigDecimal, i64, i64, i64);

async fn tokens(pool: &PgPool, period: &str) -> Vec<TokenRow> {
    sqlx::query_as(
        "SELECT mint, bucket, volume_usd, pools, pool_hours, pool_hours_valued
           FROM analytic_token_periods WHERE period = $1 ORDER BY mint",
    )
    .bind(period)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Mutation: the active pools summed hour by hour — P, active two hours, counted twice.
#[sqlx::test(migrations = "./migrations")]
async fn a_pool_active_two_hours_counts_once(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    let q = setup_pool(&pool, 2, 101, 103).await;
    tranche(&pool, &p, 0, Some(10)).await;
    tranche(&pool, &p, 1, Some(20)).await;
    tranche(&pool, &q, 1, Some(5)).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 1)).await.unwrap();

    for period in ["24h", "7d", "30d"] {
        assert_eq!(active_pools(&pool, period, 1).await, Some(2), "{period}");
    }
}

/// Mutation: the protocols taken from the facts — DLMM, without a pool, gets no
/// row, and its measured zero reads as never counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocol_without_an_active_pool_counts_zero(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    tranche(&pool, &p, 0, Some(10)).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 0)).await.unwrap();

    let dlmm: Vec<(String, i64)> = sqlx::query_as(
        "SELECT period, active_pools FROM analytic_protocol_periods
          WHERE protocol = 'meteora_dlmm' ORDER BY period",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        dlmm,
        [("24h".into(), 0), ("30d".into(), 0), ("7d".into(), 0)]
    );
}

/// Mutation: the volume written unrounded — past 28 digits the cursor, a
/// `Decimal`, no longer equals it, and the next page serves the token again.
#[sqlx::test(migrations = "./migrations")]
async fn the_next_page_starts_after_a_token_whose_volume_has_many_digits(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    let q = setup_pool(&pool, 2, 103, 104).await;
    for (address, h) in [(&p, 0), (&q, 0)] {
        tranche(&pool, address, h, Some(1)).await;
    }
    sqlx::query(
        "UPDATE analytic_pool_5min
            SET volume_in_a_usd = '1.0000000000000000000000000000000000000000'::NUMERIC / 3",
    )
    .execute(&pool)
    .await
    .unwrap();
    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 0)).await.unwrap();
    let feed = PgTokenActivityRepository::new(as_role(&pool, "yog_api").await);

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = feed.list(Window::Day, cursor, 1).await.unwrap().unwrap();
        seen.extend(page.page.items.iter().map(|t| t.mint));
        let Some(last) = page.page.items.last() else {
            break;
        };
        cursor = Some(TokenActivityCursor {
            last_hour: page.last_hour,
            volume_usd: last.volume_usd,
            mint: last.mint,
        });
        if page.page.is_last {
            break;
        }
    }
    let served = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!((served, seen.len()), (4, 4), "{seen:?}");
}

/// Mutation: the window's lower bound taken inclusive — the pool last active 24
/// hours before the frozen hour counts in its 24h window.
#[sqlx::test(migrations = "./migrations")]
async fn a_window_starts_after_the_hour_a_window_length_back(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    let q = setup_pool(&pool, 2, 101, 103).await;
    tranche(&pool, &p, 0, Some(10)).await;
    tranche(&pool, &q, 24, Some(10)).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 24)).await.unwrap();

    assert_eq!(active_pools(&pool, "24h", 24).await, Some(1));
    assert_eq!(active_pools(&pool, "7d", 24).await, Some(2));
    let day: Vec<_> = tokens(&pool, "24h")
        .await
        .into_iter()
        .map(|t| t.0)
        .collect();
    let mut expected = vec![pk(101).to_string(), pk(103).to_string()];
    expected.sort();
    assert_eq!(day, expected);
}

/// Mutations: the b side forgotten (Y and Z vanish), the pools of a token counted
/// without DISTINCT (X has 3).
#[sqlx::test(migrations = "./migrations")]
async fn a_token_is_the_sum_of_the_pools_that_hold_it(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    let q = setup_pool(&pool, 2, 101, 103).await;
    tranche(&pool, &p, 0, Some(10)).await;
    tranche(&pool, &p, 1, Some(20)).await;
    tranche(&pool, &q, 1, None).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 1)).await.unwrap();

    let mut expected = vec![
        (pk(101).to_string(), hour(1), BigDecimal::from(30), 2, 3, 2),
        (pk(102).to_string(), hour(1), BigDecimal::from(30), 1, 2, 2),
        (pk(103).to_string(), hour(1), BigDecimal::from(0), 1, 1, 0),
    ];
    expected.sort();
    assert_eq!(tokens(&pool, "24h").await, expected);
}

/// Mutation: the rows of the previous freeze kept beside the new ones.
#[sqlx::test(migrations = "./migrations")]
async fn a_freeze_replaces_the_tokens_of_the_last_one(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    tranche(&pool, &p, 0, Some(10)).await;
    tranche(&pool, &p, 1, Some(20)).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 0)).await.unwrap();
    repository.write_cycle(&cycle(1, 1)).await.unwrap();

    let rows = tokens(&pool, "24h").await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.1 == hour(1)), "{rows:?}");
    assert_eq!(rows[0].2, BigDecimal::from(30));
}

/// Mutation: the births counted on `pools.first_seen_at` — R, never initialized,
/// counts, and all three land in the hour they were first seen.
#[sqlx::test(migrations = "./migrations")]
async fn a_birth_is_an_initialize_pool_and_nothing_else(pool: PgPool) {
    let p = setup_pool(&pool, 1, 101, 102).await;
    let p2 = setup_pool(&pool, 2, 101, 103).await;
    setup_pool(&pool, 3, 101, 104).await;
    tranche(&pool, &p, 1, Some(10)).await;
    initialize(&pool, &p, hour(1) + Duration::minutes(3)).await;
    initialize(&pool, &p2, hour(1) + Duration::minutes(40)).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle(0, 1)).await.unwrap();

    let births: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
        "SELECT bucket, births FROM analytic_protocol_births_hourly
          WHERE protocol = 'meteora_damm_v2' ORDER BY bucket",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(births, [(hour(1), 2)]);
}
