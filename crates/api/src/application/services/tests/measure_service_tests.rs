//! Unit tests for `MeasureService`: the span each read covers, the liquidity's own coverage,
//! and the reads a span not covered never makes.

use std::sync::Arc;

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{FrozenHours, LiquidityMoveCursor, LiquidityMoveOrder, Protocol, Window};

use super::super::PoolMovesRead;

use crate::testing::{MockMeasureRepo, make_pool, measure_service, pk};

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
}

/// Facts from the 13th at 13:00 through the 20th at 12:00: 24h and 7d are covered, 30d is not.
fn hours() -> FrozenHours {
    FrozenHours {
        first: at(13, 13),
        last_frozen: at(20, 12),
    }
}

fn reads(repo: &MockMeasureRepo) -> Vec<&'static str> {
    repo.reads.lock().unwrap().iter().map(|r| r.0).collect()
}

/// Mutation: the `is_covered` guard dropped — a 30-day total over seven days of facts.
#[tokio::test]
async fn a_swap_total_over_hours_the_facts_lack_is_not_read() {
    let repo = Arc::new(MockMeasureRepo::default());
    let svc = measure_service(Some(hours()), repo.clone(), vec![]);

    let month = svc.swap_totals(Window::Month, None, None).await.unwrap();
    let week = svc
        .swap_totals(Window::Week, None, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert!(month.items.is_empty());
    assert_eq!(week.span.unwrap().as_of, at(20, 13));
    assert_eq!(
        *repo.reads.lock().unwrap(),
        vec![(
            "swap_totals",
            week.span.unwrap(),
            Some(Protocol::MeteoraDammV2)
        )]
    );
}

/// Mutation: the liquidity's coverage left at the facts' — a span its liquidity starts inside
/// summed as whole; or an unmeasured liquidity covered from the facts' first hour.
#[tokio::test]
async fn the_liquidity_is_covered_from_its_own_first_hour() {
    let measured = Arc::new(MockMeasureRepo {
        since: Some(at(20, 0)),
        ..MockMeasureRepo::default()
    });
    let unmeasured = Arc::new(MockMeasureRepo::default());
    let svc = measure_service(Some(hours()), measured.clone(), vec![]);
    let none = measure_service(Some(hours()), unmeasured.clone(), vec![]);

    let day = svc
        .liquidity_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let not_yet = none
        .liquidity_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(day.span.unwrap().covered_since, at(20, 0));
    assert!(reads(&measured).is_empty(), "{:?}", reads(&measured));
    assert_eq!(
        not_yet.span.unwrap().covered_since,
        not_yet.span.unwrap().as_of
    );
    assert!(reads(&unmeasured).is_empty());
}

/// Mutation: a pool's moves read over the facts' coverage — read before its liquidity was
/// measured.
#[tokio::test]
async fn a_pools_moves_wait_for_its_protocols_liquidity() {
    let pool = make_pool(pk(1), pk(2), pk(3));
    let early = Arc::new(MockMeasureRepo {
        since: Some(at(13, 13)),
        ..MockMeasureRepo::default()
    });
    let late = Arc::new(MockMeasureRepo {
        since: Some(at(20, 0)),
        ..MockMeasureRepo::default()
    });

    measure_service(Some(hours()), early.clone(), vec![])
        .pool_liquidity_moves(Window::Day, &pool, LiquidityMoveOrder::Amount, None, 5)
        .await
        .unwrap();
    measure_service(Some(hours()), late.clone(), vec![])
        .pool_liquidity_moves(Window::Day, &pool, LiquidityMoveOrder::Amount, None, 5)
        .await
        .unwrap();

    assert_eq!(reads(&early), vec!["pool_moves"]);
    assert!(reads(&late).is_empty());
}

/// Mutation: the TVL read at the open hour, or its series from the window's first hour — the
/// window's start reading lost.
#[tokio::test]
async fn the_tvl_reads_the_last_frozen_hour_and_the_series_from_the_windows_start() {
    let repo = Arc::new(MockMeasureRepo::default());
    let svc = measure_service(Some(hours()), repo.clone(), vec![]);

    svc.tvl(None).await.unwrap();
    let day = svc.tvl_hourly(Window::Day, None).await.unwrap();

    let instants = repo.instants.lock().unwrap().clone();
    assert_eq!(instants[0], ("tvl", at(20, 12), None));
    assert_eq!(
        instants[1],
        (
            "tvl_hourly",
            day.span.unwrap().from - Duration::hours(1),
            None
        )
    );
}

/// Mutation: the cursor's instant, window or pool not compared — a page follows one read
/// before a freeze, over another window or of another pool.
#[tokio::test]
async fn a_cursor_of_another_list_or_before_the_last_freeze_is_refused() {
    let pool = make_pool(pk(1), pk(2), pk(3));
    let repo = Arc::new(MockMeasureRepo {
        since: Some(at(13, 13)),
        ..MockMeasureRepo::default()
    });
    let svc = measure_service(Some(hours()), repo.clone(), vec![]);
    let cursor = |as_of, window, address| LiquidityMoveCursor {
        pool: address,
        window,
        as_of,
        order: LiquidityMoveOrder::Amount,
        amount_usd: Decimal::ONE,
        at: at(20, 12),
        signature: "sig".to_owned(),
        event_index: 0,
    };
    let read =
        |c| svc.pool_liquidity_moves(Window::Day, &pool, LiquidityMoveOrder::Amount, Some(c), 5);

    let stale = read(cursor(at(20, 12), Window::Day, pk(1))).await.unwrap();
    let other_window = read(cursor(at(20, 13), Window::Week, pk(1))).await.unwrap();
    let other_pool = read(cursor(at(20, 13), Window::Day, pk(7))).await.unwrap();
    let current = read(cursor(at(20, 13), Window::Day, pk(1))).await.unwrap();

    assert!(matches!(stale, PoolMovesRead::CursorRefused));
    assert!(matches!(other_window, PoolMovesRead::CursorRefused));
    assert!(matches!(other_pool, PoolMovesRead::CursorRefused));
    assert!(matches!(current, PoolMovesRead::Page(_)));
    assert_eq!(reads(&repo), vec!["pool_moves"]);
}

/// Mutation: the `is_covered` guard dropped from the discoveries — a 30-day part over seven
/// days of facts.
#[tokio::test]
async fn the_discoveries_over_hours_the_facts_lack_are_not_read() {
    let repo = Arc::new(MockMeasureRepo::default());
    let svc = measure_service(Some(hours()), repo.clone(), vec![]);

    svc.pool_discoveries(Window::Month, None).await.unwrap();
    svc.pool_discoveries(Window::Week, None).await.unwrap();

    let instants = repo.instants.lock().unwrap().clone();
    assert_eq!(instants, vec![("discoveries", at(20, 12), None)]);
}
