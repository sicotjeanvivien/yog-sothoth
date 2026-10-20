//! `PgProtocolActivityRepository` and `PgTokenActivityRepository`, read under
//! `yog_api` from the tables as yog-analytic leaves them, seeded here directly.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::domain::{
    ProtocolActivityLookup, Span, TokenActivityCursor, TokenActivityFeed, Window,
};
use yog_persistence::{PgProtocolActivityRepository, PgTokenActivityRepository};

use super::helpers::{as_role, pk};

/// The last frozen hour of every test: 12:00, so a day of 24 hours never starts at midnight.
fn last() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap()
}

/// `hours` hours written through [`last`], frozen, plus the open hour after it,
/// each with one pool-hour worth $1 — the open one $1 000.
async fn seed_hours(pool: &PgPool, hours: i32) {
    sqlx::query(
        "INSERT INTO analytic_hours (bucket, frozen_at)
         SELECT h, CASE WHEN h <= $1 THEN $1 + INTERVAL '2 hours' END
           FROM generate_series($1 - make_interval(hours => $2 - 1),
                                $1 + INTERVAL '1 hour', INTERVAL '1 hour') AS h",
    )
    .bind(last())
    .bind(hours)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO analytic_protocol_hourly
           (protocol, bucket, volume_usd, fees_usd, lp_fees_usd, protocol_fees_usd,
            referral_fees_usd, swap_count, active_pools, active_pools_valued)
         SELECT 'meteora_damm_v2', bucket, CASE WHEN frozen_at IS NULL THEN 1000 ELSE 1 END,
                0, 0, 0, 0, 1, 1, 1
           FROM analytic_hours",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn period(pool: &PgPool, window: &str, at: DateTime<Utc>, active_pools: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_periods (protocol, period, bucket, active_pools)
         VALUES ('meteora_damm_v2', $1, $2, $3)",
    )
    .bind(window)
    .bind(at)
    .bind(active_pools)
    .execute(pool)
    .await
    .unwrap();
}

async fn lookup(pool: &PgPool) -> (PgProtocolActivityRepository, Span, Span) {
    let repository = PgProtocolActivityRepository::new(as_role(pool, "yog_api").await);
    let hours = repository.frozen_hours().await.unwrap().unwrap();
    (
        repository,
        Span::of(hours, Window::Day, None),
        Span::of(hours, Window::Week, None),
    )
}

/// Mutation: `frozen_hours` read without `frozen_at` — the open hour's $1 000 counted.
#[sqlx::test(migrations = "./migrations")]
async fn no_read_counts_the_open_hour(pool: PgPool) {
    seed_hours(&pool, 30).await;
    let (repository, day, _) = lookup(&pool).await;

    assert_eq!(day.as_of, last() + Duration::hours(1));
    let rows = repository.activity(&day, Window::Day).await.unwrap();
    assert_eq!(rows[0].volume_usd, Some(Decimal::from(24)));
    assert_eq!(rows[0].pool_hours, 24);
    let bars = repository.activity_hourly(&day).await.unwrap();
    assert_eq!(bars.len(), 24);
    assert_eq!(bars.last().unwrap().from, last());
}

/// Mutation: the days cut at midnight — eight bars, the first and last partial.
#[sqlx::test(migrations = "./migrations")]
async fn a_day_is_24_hours_ending_at_as_of_and_the_days_add_up_to_the_window(pool: PgPool) {
    seed_hours(&pool, 24 * 8).await;
    let (repository, _, week) = lookup(&pool).await;

    let bars = repository.activity_daily(&week).await.unwrap();
    let total = repository.activity(&week, Window::Week).await.unwrap();

    assert_eq!(bars.len(), 7);
    assert!(
        bars.iter()
            .all(|bar| bar.volume_usd == Some(Decimal::from(24))),
        "{bars:?}"
    );
    assert_eq!(bars[0].from, week.from);
    assert_eq!(total[0].volume_usd, Some(Decimal::from(168)));
}

/// Mutation: the `covered_since` bound dropped — a first day half outside the facts is drawn.
#[sqlx::test(migrations = "./migrations")]
async fn a_day_the_facts_do_not_hold_whole_is_left_out(pool: PgPool) {
    seed_hours(&pool, 24 * 6 + 12).await;
    let (repository, _, week) = lookup(&pool).await;

    let bars = repository.activity_daily(&week).await.unwrap();

    assert_eq!(bars.len(), 6);
    assert_eq!(bars[0].from, week.from + Duration::days(1));
}

/// Mutation: the active pools joined without the span's last hour — every
/// window written, 10 and 7, for each span.
#[sqlx::test(migrations = "./migrations")]
async fn the_previous_window_reads_the_pools_written_a_window_earlier(pool: PgPool) {
    seed_hours(&pool, 24 * 8).await;
    period(&pool, "24h", last(), 10).await;
    period(&pool, "24h", last() - Duration::hours(24), 7).await;
    let repository = PgProtocolActivityRepository::new(as_role(&pool, "yog_api").await);
    let hours = repository.frozen_hours().await.unwrap().unwrap();

    let now = Span::of(hours, Window::Day, None);
    let before = Span::of(hours, Window::Day, Some(Window::Day));
    let week_before = Span::of(hours, Window::Day, Some(Window::Week));
    let read = |span: Span| {
        let repository = &repository;
        async move {
            let rows = repository.activity(&span, Window::Day).await.unwrap();
            rows.iter().map(|row| row.active_pools).collect::<Vec<_>>()
        }
    };

    assert_eq!(read(now).await, [Some(10)]);
    assert_eq!(read(before).await, [Some(7)]);
    assert_eq!(read(week_before).await, [None]);
}

/// Mutation: births joined to the facts — an hour of births without a swap is lost.
#[sqlx::test(migrations = "./migrations")]
async fn births_without_a_swap_are_counted(pool: PgPool) {
    seed_hours(&pool, 30).await;
    sqlx::query("DELETE FROM analytic_protocol_hourly")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO analytic_protocol_births_hourly (protocol, bucket, births)
         VALUES ('meteora_damm_v2', $1, 3), ('meteora_damm_v2', $2, 100)",
    )
    .bind(last())
    .bind(last() + Duration::hours(1))
    .execute(&pool)
    .await
    .unwrap();
    let (repository, day, _) = lookup(&pool).await;

    let rows = repository.activity(&day, Window::Day).await.unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].births, rows[0].swap_count), (3, 0));
}

async fn token(pool: &PgPool, mint: u8, volume: i64, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO analytic_token_periods
           (period, mint, bucket, volume_usd, pools, pool_hours, pool_hours_valued)
         VALUES ('24h', $1, $2, $3, 1, 1, 1)",
    )
    .bind(pk(mint).to_string())
    .bind(at)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutations: the cursor's tie on volume broken the other way (a token served
/// twice, another skipped), or the identity left unjoined.
#[sqlx::test(migrations = "./migrations")]
async fn the_tokens_page_by_volume_then_mint_with_their_identity(pool: PgPool) {
    for (mint, volume) in [(1, 30), (2, 20), (3, 20), (4, 5)] {
        token(&pool, mint, volume, last()).await;
    }
    sqlx::query(
        "INSERT INTO token_metadata (mint, symbol, decimals, fetched_at, last_refresh_at)
         VALUES ($1, 'ONE', 9, now(), now())",
    )
    .bind(pk(1).to_string())
    .execute(&pool)
    .await
    .unwrap();
    let feed = PgTokenActivityRepository::new(as_role(&pool, "yog_api").await);

    let first = feed.list(Window::Day, None, 2).await.unwrap().unwrap();
    let cursor = first.page.items.last().map(|t| TokenActivityCursor {
        last_hour: first.last_hour,
        volume_usd: t.volume_usd,
        mint: t.mint,
    });
    let second = feed.list(Window::Day, cursor, 2).await.unwrap().unwrap();
    let mints = |items: &[yog_core::domain::TokenActivity]| -> Vec<_> {
        items.iter().map(|t| t.mint).collect()
    };

    let (low, high) = if pk(2) < pk(3) { (2, 3) } else { (3, 2) };
    assert_eq!(first.last_hour, last());
    assert_eq!(mints(&first.page.items), [pk(1), pk(low)]);
    assert_eq!(mints(&second.page.items), [pk(high), pk(4)]);
    assert!(!first.page.is_last && second.page.is_last && first.page.prev_cursor.is_none());
    let symbol = first.page.items[0]
        .metadata
        .as_ref()
        .and_then(|m| m.symbol.as_deref());
    assert_eq!(symbol, Some("ONE"));
    assert!(first.page.items[1].metadata.is_none());
}

/// Mutation: the hour taken from the rows — an empty page past the last token
/// would lose the hour of its ranking, and a ranking never written read as one.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_past_the_last_token_still_names_its_ranking(pool: PgPool) {
    token(&pool, 1, 30, last()).await;
    let feed = PgTokenActivityRepository::new(as_role(&pool, "yog_api").await);

    let past = TokenActivityCursor {
        last_hour: last(),
        volume_usd: Decimal::from(1),
        mint: pk(9),
    };
    let page = feed
        .list(Window::Day, Some(past), 10)
        .await
        .unwrap()
        .unwrap();
    let week = feed.list(Window::Week, None, 10).await.unwrap();

    assert_eq!(page.last_hour, last());
    assert!(page.page.items.is_empty() && page.page.is_last);
    assert!(week.is_none());
}

/// Mutation: the TVL read at the last frozen hour only — after a catch-up that
/// hour has no measure, and the TVL vanishes.
#[sqlx::test(migrations = "./migrations")]
async fn the_tvl_is_the_last_measured_at_or_before_the_hour(pool: PgPool) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl (protocol, bucket, tvl_usd, pools_priced, measured_at)
         VALUES ('meteora_damm_v2', $1, 100, 3, $1), ('meteora_damm_v2', $2, 999, 3, $2)",
    )
    .bind(last() - Duration::hours(3))
    .bind(last() + Duration::hours(1))
    .execute(&pool)
    .await
    .unwrap();
    let repository = PgProtocolActivityRepository::new(as_role(&pool, "yog_api").await);

    let tvl = repository.tvl_latest(last()).await.unwrap();

    assert_eq!(tvl.len(), 1);
    assert_eq!(tvl[0].tvl_usd, Some(Decimal::from(100)));
    assert_eq!(tvl[0].at, last() - Duration::hours(2));
}
