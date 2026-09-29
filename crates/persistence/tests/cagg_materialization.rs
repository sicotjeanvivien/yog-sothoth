//! `yog_cagg_materialization_progress()` (migration 013): which raw rows each
//! continuous aggregate has not materialised yet, and who may ask.
//!
//! The function is the whole distinction between the two stalls `yog-signals`
//! must not confuse: an **ingestion** halt leaves nothing pending once the
//! refresh catches up, a **materialisation** halt leaves rows pending that only
//! grow older. Its filter — rows at or past the watermark — is what the first
//! two tests pin; the watermark's position after a bounded refresh is
//! TimescaleDB's, measured here rather than assumed.

use super::helpers::{INSUFFICIENT_PRIVILEGE, apply_setup_roles, pk, sqlstate};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::PgPool;
use yog_core::domain::{AggregateMaterialization, MaterializationRepository};
use yog_persistence::PgMaterializationRepository;

const SWAPS: &str = "meteora_damm_v2_swap_events_hourly";

/// 10 January 2026, `hour:minute` UTC — far enough back that nothing about the
/// test depends on the clock.
fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 10, hour, minute, 0).unwrap()
}

async fn seed_pool(pool: &PgPool) -> String {
    let address = pk(1).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&address)
    .bind(pk(2).to_string())
    .bind(pk(3).to_string())
    .execute(pool)
    .await
    .unwrap();
    address
}

async fn swap_at(pool: &PgPool, address: &str, signature: &str, timestamp: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'a_to_b', 1000, 1000, 0, 0, 0, 10, 0, 0, 0, true, $3, 0, 0)",
    )
    .bind(address)
    .bind(signature)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

/// A bounded refresh over the whole day. `refresh_continuous_aggregate` cannot
/// run inside a transaction; `sqlx::test` hands out a pool, so it runs here.
async fn refresh_the_day(pool: &PgPool) {
    sqlx::query("CALL refresh_continuous_aggregate($1, $2, $3)")
        .bind(SWAPS)
        .bind(at(0, 0))
        .bind(Utc.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap())
        .execute(pool)
        .await
        .expect("a bounded refresh must materialise the seeded day");
}

async fn progress_of(pool: &PgPool, aggregate: &str) -> AggregateMaterialization {
    let progress = PgMaterializationRepository::new(pool.clone())
        .progress()
        .await
        .expect("read the progress");
    progress
        .into_iter()
        .find(|m| m.aggregate == aggregate)
        .unwrap_or_else(|| panic!("{aggregate} is missing from the progress"))
}

/// Every aggregate the catalog holds is reported — none listed by hand.
#[sqlx::test]
async fn every_aggregate_is_reported_and_an_empty_one_has_nothing_pending(pool: PgPool) {
    let progress = PgMaterializationRepository::new(pool.clone())
        .progress()
        .await
        .expect("read the progress");

    let names: Vec<&str> = progress.iter().map(|m| m.aggregate.as_str()).collect();
    assert_eq!(
        names,
        [
            "meteora_damm_v2_claim_position_fee_events_hourly",
            "meteora_damm_v2_claim_reward_events_hourly",
            "meteora_damm_v2_liquidity_events_hourly",
            "meteora_damm_v2_swap_events_hourly",
        ]
    );
    for materialization in &progress {
        assert_eq!(
            (materialization.watermark, materialization.oldest_pending_at),
            (None, None),
            "{}: no bucket, no raw row",
            materialization.aggregate
        );
    }
}

/// Rows arrived, no refresh ever ran: the oldest row is what waits. The
/// watermark reads as `None` — TimescaleDB's own sentinel for "never" is a
/// date (4714 BC under 2.30.1), which the function must not let through.
#[sqlx::test]
async fn before_any_refresh_the_oldest_raw_row_is_pending(pool: PgPool) {
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-late", at(11, 20)).await;
    swap_at(&pool, &address, "sig-early", at(10, 15)).await;

    let swaps = progress_of(&pool, SWAPS).await;
    assert_eq!(swaps.watermark, None);
    assert_eq!(swaps.oldest_pending_at, Some(at(10, 15)));
}

/// The refresh caught up and no row came after: this is what an ingestion
/// halt looks like, and nothing may be pending. Also the premise the lag rests
/// on, measured: the watermark stops at the end of the last bucket that held a
/// row, not at the end of the refreshed window.
#[sqlx::test]
async fn once_refreshed_nothing_is_pending_and_the_watermark_stops_at_the_data(pool: PgPool) {
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-early", at(10, 15)).await;
    swap_at(&pool, &address, "sig-late", at(11, 20)).await;
    refresh_the_day(&pool).await;

    let swaps = progress_of(&pool, SWAPS).await;
    assert_eq!(swaps.watermark, Some(at(12, 0)));
    assert_eq!(swaps.oldest_pending_at, None);
}

/// A row landing after the refresh waits — and one landing exactly at the
/// watermark does too: the watermark is the END of the last bucket, so a row
/// at it opens the next one, which nothing has materialised.
#[sqlx::test]
async fn a_row_at_or_past_the_watermark_is_pending(pool: PgPool) {
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-early", at(10, 15)).await;
    swap_at(&pool, &address, "sig-late", at(11, 20)).await;
    refresh_the_day(&pool).await;

    swap_at(&pool, &address, "sig-after", at(13, 30)).await;
    assert_eq!(
        progress_of(&pool, SWAPS).await.oldest_pending_at,
        Some(at(13, 30))
    );

    swap_at(&pool, &address, "sig-on-the-edge", at(12, 0)).await;
    assert_eq!(
        progress_of(&pool, SWAPS).await.oldest_pending_at,
        Some(at(12, 0))
    );
}

/// `yog_signals` may call it; another runtime role may not. Proven under the
/// roles themselves: a privilege is only shown by the statement it lets
/// through or stops.
#[sqlx::test]
async fn only_yog_signals_may_read_the_progress(pool: PgPool) {
    apply_setup_roles(&pool).await;

    // ⚠️ The function is SECURITY DEFINER, so it runs with its OWNER's rights.
    // Here the migrations ran as the admin superuser, who bypasses every
    // check: the catalog reads and `cagg_watermark` would pass whatever
    // TimescaleDB grants. In production the owner is `yog_migrate`, which owns
    // the schema — so the test hands the function to it, and gives it the read
    // on the tables that ownership gives it there.
    sqlx::raw_sql(
        "ALTER FUNCTION yog_cagg_materialization_progress() OWNER TO yog_migrate;
         GRANT SELECT ON ALL TABLES IN SCHEMA public TO yog_migrate;",
    )
    .execute(&pool)
    .await
    .expect("hand the function to its production owner");

    // A row, so the function reads a real chunk under its owner's rights — an
    // empty hypertable has no chunk, and the read of one would never be tried.
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-privileges", at(10, 15)).await;

    let mut conn = pool.acquire().await.unwrap();

    sqlx::query("SET ROLE yog_signals")
        .execute(&mut *conn)
        .await
        .unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM yog_cagg_materialization_progress()")
        .fetch_one(&mut *conn)
        .await
        .expect("yog_signals must be able to read the progress");
    assert_eq!(rows, 4);

    sqlx::query("SET ROLE yog_api")
        .execute(&mut *conn)
        .await
        .unwrap();
    let refused = sqlx::query("SELECT count(*) FROM yog_cagg_materialization_progress()")
        .execute(&mut *conn)
        .await
        .expect_err("yog_api must not read the progress");
    assert_eq!(sqlstate(&refused), INSUFFICIENT_PRIVILEGE);

    sqlx::query("RESET ROLE").execute(&mut *conn).await.unwrap();
}
