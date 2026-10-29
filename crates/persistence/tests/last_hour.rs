//! The last hour under `yog_api`: the twelve tranches through the last one written, read from
//! `analytic_pool_5min` and the creations themselves.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{
    PoolCreationsLookup, Protocol, ProtocolActivityLookup, SwapActivityLookup, TokenActivityFeed,
    TranchePeriod, Window,
};
use yog_persistence::{
    PgPoolCreationsRepository, PgProtocolActivityRepository, PgSwapActivityRepository,
    PgTokenActivityRepository,
};

use super::helpers::{pk, setup_roles_and_connect_as};

/// The end of the last tranche written: a quarter past, two hours back.
fn through() -> DateTime<Utc> {
    (Utc::now() - TimeDelta::hours(2))
        .duration_trunc(TimeDelta::hours(1))
        .unwrap()
        + TimeDelta::minutes(15)
}

fn minutes_before(minutes: i64) -> DateTime<Utc> {
    through() - TimeDelta::minutes(minutes)
}

/// Every tranche of the three hours before `through()` written.
async fn write_tranches(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO analytic_tranches (bucket, written_at)
         SELECT generate_series($1::TIMESTAMPTZ - INTERVAL '3 hours',
                                $1::TIMESTAMPTZ - INTERVAL '5 minutes', INTERVAL '5 minutes'), now()",
    )
    .bind(through())
    .execute(pool)
    .await
    .unwrap();
}

/// A pool of DAMM v2 trading `a` against `b`.
async fn seed_pool(pool: &PgPool, address: &str, a: &str, b: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(address)
    .bind(a)
    .bind(b)
    .execute(pool)
    .await
    .unwrap();
}

/// One pool's tranche at `bucket`: `swaps` swaps worth `usd`, valued unless `usd` is `None`.
async fn tranche(
    pool: &PgPool,
    address: &str,
    bucket: DateTime<Utc>,
    swaps: i64,
    usd: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO analytic_pool_5min
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd, volume_in_b_usd,
            fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd)
         VALUES ($1, $2, 'meteora_damm_v2', $3, $4, $5, 0, $5, $5, 0, 0)",
    )
    .bind(address)
    .bind(bucket)
    .bind(swaps)
    .bind(usd.is_some())
    .bind(usd.map(BigDecimal::from))
    .execute(pool)
    .await
    .unwrap();
}

async fn last_hour(pool: &PgPool) -> TranchePeriod {
    let repo = PgProtocolActivityRepository::new(setup_roles_and_connect_as(pool, "yog_api").await);
    TranchePeriod::hour_through(repo.written_tranches().await.unwrap().unwrap(), None)
}

/// Against the hour cut at 11 or 13 tranches, or pools counted per tranche: the tranche just
/// before the hour and the one at its end stay out, a pool active in two tranches counts once,
/// and an unvalued tranche adds nothing to the volume.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hour_sums_its_twelve_tranches_and_counts_its_pools_once(pool: PgPool) {
    write_tranches(&pool).await;
    let (x, y) = (pk(1).to_string(), pk(2).to_string());
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    seed_pool(&pool, &y, &pk(10).to_string(), &pk(12).to_string()).await;
    tranche(&pool, &x, minutes_before(65), 1000, Some(1000)).await;
    tranche(&pool, &x, minutes_before(60), 1, Some(10)).await;
    tranche(&pool, &x, minutes_before(5), 2, Some(20)).await;
    tranche(&pool, &y, minutes_before(30), 4, None).await;
    tranche(&pool, &x, through(), 1000, Some(1000)).await;

    let period = last_hour(&pool).await;
    let repo = PgSwapActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let totals = repo
        .find_protocol_totals_in_tranches(&period, None)
        .await
        .unwrap();

    assert_eq!(period.as_of, through());
    assert_eq!(totals.len(), 1);
    let activity = &totals[0].activity;
    assert_eq!(activity.swaps, 7);
    assert_eq!(activity.volume_usd, Some(Decimal::from(30)));
    assert_eq!((activity.pool_hours, activity.pool_hours_valued), (3, 2));
    assert_eq!(totals[0].pools, Some(2));
}

/// Against a tranche without a pool left out of the series: the hour has its twelve bars, 0 where
/// nothing traded; and against the hour before read where it is not.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hour_has_twelve_bars_and_the_hour_before_its_own(pool: PgPool) {
    write_tranches(&pool).await;
    let x = pk(1).to_string();
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(&pool, &x, minutes_before(5), 2, Some(20)).await;
    tranche(&pool, &x, minutes_before(65), 3, Some(30)).await;

    let period = last_hour(&pool).await;
    let tranches = PgProtocolActivityRepository::new(pool.clone())
        .written_tranches()
        .await
        .unwrap()
        .unwrap();
    let before = TranchePeriod::hour_through(tranches, Some(Window::Hour));
    let repo = PgSwapActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let bars = repo
        .list_protocol_tranches(&period, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let previous = repo
        .find_protocol_totals_in_tranches(&before, None)
        .await
        .unwrap();

    assert_eq!(bars.len(), 12);
    assert_eq!(bars[0].from, minutes_before(60));
    assert_eq!(bars[11].value.swaps, 2);
    assert_eq!(bars[5].value.swaps, 0);
    assert_eq!(bars[5].value.volume_usd, Some(Decimal::ZERO));
    assert_eq!(previous[0].activity.swaps, 3);
}

/// Against a token's pools counted per tranche, or its hour cut at 11 or 13 tranches: the token
/// of two pools, one active twice, has two pools and their volume within the hour.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_tokens_count_their_pools_once(pool: PgPool) {
    write_tranches(&pool).await;
    let (x, y, shared) = (pk(1).to_string(), pk(2).to_string(), pk(10).to_string());
    seed_pool(&pool, &x, &shared, &pk(11).to_string()).await;
    seed_pool(&pool, &y, &shared, &pk(12).to_string()).await;
    tranche(&pool, &x, minutes_before(60), 1, Some(10)).await;
    tranche(&pool, &x, minutes_before(10), 1, Some(10)).await;
    tranche(&pool, &y, minutes_before(20), 1, Some(5)).await;
    tranche(&pool, &y, minutes_before(65), 1, Some(1000)).await;
    tranche(&pool, &y, through(), 1, Some(1000)).await;

    let period = last_hour(&pool).await;
    let repo = PgTokenActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let ranking = repo.list_in_tranches(&period, None, 10).await.unwrap();

    assert_eq!(ranking.last_hour, period.as_of);
    let first = &ranking.page.items[0];
    assert_eq!(first.mint.to_string(), shared);
    assert_eq!(first.volume_usd, Decimal::from(25));
    assert_eq!((first.pools, first.pool_hours), (2, 3));
}

/// Against the coverage taken from the tranches recorded rather than those still held: past the
/// 5-minute retention, an hour is not covered.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hour_is_covered_from_the_first_tranche_still_held(pool: PgPool) {
    write_tranches(&pool).await;
    let x = pk(1).to_string();
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(&pool, &x, minutes_before(65), 1, Some(10)).await;

    let repo =
        PgProtocolActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let tranches = repo.written_tranches().await.unwrap().unwrap();

    assert_eq!(tranches.first, minutes_before(65));
    assert_eq!(tranches.through, through());
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

/// Against the hour's creations counted outside it, or for a protocol without a creation table:
/// two creations in the hour, one before it, and none for DLMM.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_creations_are_its_own(pool: PgPool) {
    write_tranches(&pool).await;
    initialize(&pool, &pk(1).to_string(), minutes_before(70)).await;
    initialize(&pool, &pk(2).to_string(), minutes_before(40)).await;
    initialize(&pool, &pk(3).to_string(), minutes_before(1)).await;

    let period = last_hour(&pool).await;
    let repo = PgPoolCreationsRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let totals = repo.list_totals_in_tranches(&period, None).await.unwrap();
    let dlmm = repo
        .list_totals_in_tranches(&period, Some(Protocol::MeteoraDlmm))
        .await
        .unwrap();
    let bars = repo.list_tranches(&period, None).await.unwrap();

    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].pools, 2);
    assert!(dlmm.is_empty());
    assert_eq!(bars.len(), 12);
    assert_eq!(bars.iter().map(|b| b.value.pools).sum::<i64>(), 2);
}
