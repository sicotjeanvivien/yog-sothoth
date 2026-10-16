//! `yog_cagg_materialization_backlog()`: which raw rows each continuous aggregate has not
//! materialised yet, and who may ask. It tells an ingestion halt (nothing pending once the
//! refresh catches up) from a materialisation halt (pending rows that only grow older).

use super::helpers::{INSUFFICIENT_PRIVILEGE, apply_setup_roles, pk, sqlstate};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::PgPool;
use yog_core::domain::{MaterializationBacklog, MaterializationBacklogRepository};
use yog_persistence::PgMaterializationBacklogRepository;

const SWAPS: &str = "meteora_damm_v2_swap_events_hourly";

/// `hour:minute` UTC on a fixed past day, so nothing depends on the clock.
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

/// A bounded refresh over the whole day; it cannot run inside a transaction.
async fn refresh_the_day(pool: &PgPool) {
    sqlx::query("CALL refresh_continuous_aggregate($1, $2, $3)")
        .bind(SWAPS)
        .bind(at(0, 0))
        .bind(Utc.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap())
        .execute(pool)
        .await
        .expect("a bounded refresh must materialise the seeded day");
}

async fn backlog_of(pool: &PgPool, aggregate: &str) -> MaterializationBacklog {
    let backlogs = PgMaterializationBacklogRepository::new(pool.clone())
        .backlogs()
        .await
        .expect("read the backlogs");
    backlogs
        .into_iter()
        .find(|m| m.aggregate == aggregate)
        .unwrap_or_else(|| panic!("{aggregate} is missing from the backlogs"))
}

/// Every aggregate the catalog holds is reported — none listed by hand.
#[sqlx::test]
async fn every_aggregate_is_reported_and_an_empty_one_has_nothing_pending(pool: PgPool) {
    let backlogs = PgMaterializationBacklogRepository::new(pool.clone())
        .backlogs()
        .await
        .expect("read the backlogs");

    let names: Vec<&str> = backlogs.iter().map(|m| m.aggregate.as_str()).collect();
    assert_eq!(
        names,
        [
            "meteora_damm_v2_claim_position_fee_events_hourly",
            "meteora_damm_v2_claim_reward_events_hourly",
            "meteora_damm_v2_liquidity_events_hourly",
            "meteora_damm_v2_swap_events_hourly",
        ]
    );
    for backlog in &backlogs {
        assert_eq!(
            (backlog.watermark, backlog.oldest_pending_at),
            (None, None),
            "{}: no bucket, no raw row",
            backlog.aggregate
        );
    }
}

/// No refresh ever ran: the oldest row waits, and the watermark reads `None` — not
/// TimescaleDB's sentinel date for "never".
#[sqlx::test]
async fn before_any_refresh_the_oldest_raw_row_is_pending(pool: PgPool) {
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-late", at(11, 20)).await;
    swap_at(&pool, &address, "sig-early", at(10, 15)).await;

    let swaps = backlog_of(&pool, SWAPS).await;
    assert_eq!(swaps.watermark, None);
    assert_eq!(swaps.oldest_pending_at, Some(at(10, 15)));
}

/// An ingestion halt: nothing pending. ⚠️ The watermark stops at the end of the last bucket
/// that held a row, not at the end of the refreshed window.
#[sqlx::test]
async fn once_refreshed_nothing_is_pending_and_the_watermark_stops_at_the_data(pool: PgPool) {
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-early", at(10, 15)).await;
    swap_at(&pool, &address, "sig-late", at(11, 20)).await;
    refresh_the_day(&pool).await;

    let swaps = backlog_of(&pool, SWAPS).await;
    assert_eq!(swaps.watermark, Some(at(12, 0)));
    assert_eq!(swaps.oldest_pending_at, None);
}

/// A row after the refresh waits, and one exactly at the watermark too: it opens the next
/// bucket, which nothing has materialised.
#[sqlx::test]
async fn a_row_at_or_past_the_watermark_is_pending(pool: PgPool) {
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-early", at(10, 15)).await;
    swap_at(&pool, &address, "sig-late", at(11, 20)).await;
    refresh_the_day(&pool).await;

    swap_at(&pool, &address, "sig-after", at(13, 30)).await;
    assert_eq!(
        backlog_of(&pool, SWAPS).await.oldest_pending_at,
        Some(at(13, 30))
    );

    swap_at(&pool, &address, "sig-on-the-edge", at(12, 0)).await;
    assert_eq!(
        backlog_of(&pool, SWAPS).await.oldest_pending_at,
        Some(at(12, 0))
    );
}

/// `yog_signals` may call it; another runtime role may not — proven under the roles.
#[sqlx::test]
async fn only_yog_signals_may_read_the_backlogs(pool: PgPool) {
    apply_setup_roles(&pool).await;

    // ⚠️ SECURITY DEFINER runs as its owner, a superuser here: the test hands it to
    // `yog_migrate`, the production owner, with the reads ownership gives it there.
    sqlx::raw_sql(
        "ALTER FUNCTION yog_cagg_materialization_backlog() OWNER TO yog_migrate;
         GRANT SELECT ON ALL TABLES IN SCHEMA public TO yog_migrate;",
    )
    .execute(&pool)
    .await
    .expect("hand the function to its production owner");

    // A row, so a real chunk is read under the owner's rights.
    let address = seed_pool(&pool).await;
    swap_at(&pool, &address, "sig-privileges", at(10, 15)).await;

    let mut conn = pool.acquire().await.unwrap();

    sqlx::query("SET ROLE yog_signals")
        .execute(&mut *conn)
        .await
        .unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM yog_cagg_materialization_backlog()")
        .fetch_one(&mut *conn)
        .await
        .expect("yog_signals must be able to read the backlogs");
    assert_eq!(rows, 4);

    sqlx::query("SET ROLE yog_api")
        .execute(&mut *conn)
        .await
        .unwrap();
    let refused = sqlx::query("SELECT count(*) FROM yog_cagg_materialization_backlog()")
        .execute(&mut *conn)
        .await
        .expect_err("yog_api must not read the backlogs");
    assert_eq!(sqlstate(&refused), INSUFFICIENT_PRIVILEGE);

    sqlx::query("RESET ROLE").execute(&mut *conn).await.unwrap();
}
