//! `PgAnalyticRepository`, written under `yog_analytic` itself: the facts and
//! the protocol aggregate against the activity view, the freeze, the TVL.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{INSUFFICIENT_PRIVILEGE, apply_setup_roles, pk, price_mint, sqlstate};

/// A pool on the test's database whose connections act as `yog_analytic`.
async fn as_analytic(pool: &PgPool) -> PgPool {
    apply_setup_roles(pool).await;
    let options = (*pool.connect_options())
        .clone()
        .options([("role", "yog_analytic")]);
    let analytic = PgPool::connect_with(options).await.unwrap();
    let user: String = sqlx::query_scalar("SELECT current_user::TEXT")
        .fetch_one(&analytic)
        .await
        .unwrap();
    assert_eq!(user, "yog_analytic");
    analytic
}

/// One instant per test, at the precision Postgres keeps: two `Utc::now()`
/// on either side of an hour would put a swap and its cycle in different hours.
fn now() -> DateTime<Utc> {
    Utc::now()
        .duration_trunc(Duration::microseconds(1))
        .unwrap()
}

fn hours_before(now: DateTime<Utc>, hours: i64) -> DateTime<Utc> {
    (now - Duration::hours(hours))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

/// The cycle at `now` that writes every hour from `first`, and freezes the
/// closed ones.
fn cycle_from(now: DateTime<Utc>, first: DateTime<Utc>) -> CycleWrite {
    CycleWrite::plan(
        now,
        AnalyticProgress {
            first_hour: Some(first),
            last_frozen: None,
        },
    )
}

/// Token A unlisted, token B priced when `price_b` is given, as yog-context
/// leaves most pools.
async fn setup_pool(pool: &PgPool, seed: u8, price_b: Option<&str>) -> String {
    let address = pk(seed).to_string();
    let (mint_a, mint_b) = (format!("mint_a_{seed}"), format!("mint_b_{seed}"));
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
    if let Some(price) = price_b {
        price_mint(pool, &mint_b, price).await;
    }
    address
}

/// A swap each way, both fees on the out token (`collect_fee_mode = 0`), each
/// carrying a protocol and a referral share.
async fn trade(pool: &PgPool, address: &str, tag: &str, at: DateTime<Utc>) {
    for (direction, a, b, claiming, protocol, referral, fee_is_a) in [
        (
            "a_to_b",
            1_000_000_000i64,
            500_000_000i64,
            4_000_000i64,
            800_000i64,
            200_000i64,
            false,
        ),
        (
            "b_to_a",
            2_000_000_000,
            1_000_000_000,
            80_000_000,
            16_000_000,
            4_000_000,
            true,
        ),
    ] {
        sqlx::query(
            "INSERT INTO meteora_damm_v2_swap_events
               (pool_address, signature, trade_direction,
                amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
                timestamp, slot, event_index)
             VALUES ($1, $2, $3, $4, $5, 0, 0, 0, $6, $7, 0, $8, $9, $10, 0, 0)",
        )
        .bind(address)
        .bind(format!("{tag}_{direction}"))
        .bind(direction)
        .bind(a)
        .bind(b)
        .bind(claiming)
        .bind(protocol)
        .bind(referral)
        .bind(fee_is_a)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
    }
}

type PoolHour = (
    String,
    DateTime<Utc>,
    i64,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
);

async fn view_pool_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<PoolHour> {
    sqlx::query_as(
        "SELECT pool_address, bucket, swap_count, volume_usd, fees_usd,
                lp_fees_usd, protocol_fees_usd, referral_fees_usd
           FROM meteora_damm_v2_pool_hourly_activity
          WHERE swap_count IS NOT NULL AND bucket <= $1
          ORDER BY pool_address, bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn fact_pool_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<PoolHour> {
    sqlx::query_as(
        "SELECT pool_address, bucket, swap_count, volume_in_a_usd + volume_in_b_usd,
                fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd
           FROM analytic_pool_hourly
          WHERE bucket <= $1
          ORDER BY pool_address, bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

type ProtocolHour = (
    DateTime<Utc>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    i64,
    i64,
    i64,
);

async fn view_protocol_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<ProtocolHour> {
    sqlx::query_as(
        "SELECT bucket, SUM(volume_usd), SUM(fees_usd), SUM(lp_fees_usd),
                SUM(swap_count)::BIGINT, COUNT(*),
                COUNT(*) FILTER (WHERE volume_usd IS NOT NULL)
           FROM meteora_damm_v2_pool_hourly_activity
          WHERE swap_count IS NOT NULL AND bucket <= $1
          GROUP BY bucket
          ORDER BY bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn aggregate_protocol_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<ProtocolHour> {
    sqlx::query_as(
        "SELECT bucket, volume_usd, fees_usd, lp_fees_usd,
                swap_count, active_pools, active_pools_valued
           FROM analytic_protocol_hourly
          WHERE protocol = 'meteora_damm_v2' AND bucket <= $1
          ORDER BY bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Against any formula of the branch drifting from `swap_parts` (010): a
/// fee share swapped, a side dropped, a valued flag read from the wrong side.
#[sqlx::test]
async fn facts_and_aggregate_equal_the_activity_view_on_closed_hours(pool: PgPool) {
    let now = now();
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;
    trade(&pool, &priced, "p3", hours_before(now, 3)).await;
    trade(&pool, &priced, "p2", hours_before(now, 2)).await;
    trade(&pool, &unpriced, "u2", hours_before(now, 2)).await;
    let closed = hours_before(now, 2);

    let repository = PgAnalyticRepository::new(as_analytic(&pool).await);
    let cycle = cycle_from(now, hours_before(now, 3));
    repository.write_cycle(&cycle).await.unwrap();

    let view = view_pool_hours(&pool, closed).await;
    assert_eq!(view.len(), 3);
    assert_eq!(view.iter().filter(|h| h.3.is_some()).count(), 2, "{view:?}");
    assert!(view.iter().any(|h| h.7.as_ref().is_some_and(|r| r > 0)));
    assert_eq!(fact_pool_hours(&pool, closed).await, view);

    let by_protocol = view_protocol_hours(&pool, closed).await;
    assert_eq!(by_protocol.len(), 2);
    assert_eq!(aggregate_protocol_hours(&pool, closed).await, by_protocol);
}

/// Against the freeze guard of `write_cycle` removed: the frozen hour would
/// be rewritten with the late swap.
#[sqlx::test]
async fn a_swap_written_after_the_freeze_changes_nothing_of_its_hour(pool: PgPool) {
    let now = now();
    let address = setup_pool(&pool, 1, Some("180.0")).await;
    let frozen = hours_before(now, 3);
    trade(&pool, &address, "before", frozen).await;
    let current = hours_before(now, 0);
    trade(&pool, &address, "now", current).await;

    let repository = PgAnalyticRepository::new(as_analytic(&pool).await);
    let cycle = cycle_from(now, frozen);
    let first = repository.write_cycle(&cycle).await.unwrap();
    assert!(first.hours_frozen >= 1);
    let facts = fact_pool_hours(&pool, frozen).await;
    let aggregate = aggregate_protocol_hours(&pool, frozen).await;
    assert_eq!(facts[0].2, 2);

    trade(&pool, &address, "late", frozen + Duration::minutes(30)).await;
    trade(&pool, &address, "later", current).await;
    repository.write_cycle(&cycle).await.unwrap();

    assert_eq!(fact_pool_hours(&pool, frozen).await, facts);
    assert_eq!(aggregate_protocol_hours(&pool, frozen).await, aggregate);
    let open = fact_pool_hours(&pool, current).await;
    assert_eq!(
        open.last().unwrap().2,
        4,
        "the open hour is rewritten: {open:?}"
    );
}

#[sqlx::test]
async fn the_tvl_is_the_sum_of_pool_current_tvl(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;
    price_mint(&pool, "mint_a_1", "2.5").await;
    for (address, reserve_a, reserve_b) in
        [(&priced, 3_000_000i64, 7_000_000_000i64), (&unpriced, 1, 1)]
    {
        sqlx::query(
            "INSERT INTO pool_current_state
               (pool_address, protocol, last_event_at, last_event_kind, last_signature,
                reserve_a, reserve_b, last_slot, last_event_index)
             VALUES ($1, 'meteora_damm_v2', now(), 'liquidity_add', 'sig', $2, $3, 1, 0)",
        )
        .bind(address)
        .bind(reserve_a)
        .bind(reserve_b)
        .execute(&pool)
        .await
        .unwrap();
    }

    let now = now();
    let repository = PgAnalyticRepository::new(as_analytic(&pool).await);
    let cycle = cycle_from(now, hours_before(now, 0));
    repository.write_cycle(&cycle).await.unwrap();

    let expected: (Option<BigDecimal>, i64) = sqlx::query_as(
        "SELECT SUM(tvl_usd), COUNT(*) FILTER (WHERE tvl_usd IS NOT NULL) FROM pool_current_tvl",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expected, (Some("1267.5".parse::<BigDecimal>().unwrap()), 1));
    let measured: (DateTime<Utc>, Option<BigDecimal>, i64, DateTime<Utc>) = sqlx::query_as(
        "SELECT bucket, tvl_usd, pools_priced, measured_at
           FROM analytic_protocol_tvl WHERE protocol = 'meteora_damm_v2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        measured,
        (cycle.tvl_hour.unwrap(), expected.0, expected.1, cycle.at)
    );
}

#[sqlx::test]
async fn yog_analytic_cannot_write_signals(pool: PgPool) {
    let analytic = as_analytic(&pool).await;

    let err = sqlx::query(
        "INSERT INTO signals (detector, protocol, pool_address, severity, value, triggered_at)
         VALUES ('d', 'meteora_damm_v2', 'p', 'warning', 1, now())",
    )
    .execute(&analytic)
    .await
    .expect_err("yog_analytic writes the analytic tables only");

    assert_eq!(sqlstate(&err), INSUFFICIENT_PRIVILEGE);
    let message = err.as_database_error().unwrap().message().to_owned();
    assert!(message.contains("table signals"), "{message}");
}
