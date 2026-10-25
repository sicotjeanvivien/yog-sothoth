//! Unit tests for `LiquidityMoveService`: the liquidity's own coverage, the reads a period it
//! does not cover never makes, and the cursors a page refuses.

use std::sync::Arc;

use rust_decimal::Decimal;
use yog_core::domain::{LiquidityMoveCursor, LiquidityMoveOrder, Protocol, Window};

use super::{LiquidityMoveService, PoolMovesError};
use crate::testing::{
    MockLiquidityMoveRepo, at_october, frozen_hours_repo, make_frozen_hours, make_pool, pk,
};

fn service(repo: &Arc<MockLiquidityMoveRepo>) -> LiquidityMoveService {
    LiquidityMoveService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone())
}

fn reads(repo: &MockLiquidityMoveRepo) -> Vec<&'static str> {
    repo.reads.lock().unwrap().iter().map(|r| r.0).collect()
}

/// Mutation: the liquidity's coverage left at the facts' — a period its liquidity starts inside
/// summed as whole; or an unmeasured liquidity covered from the facts' first hour.
#[tokio::test]
async fn the_liquidity_is_covered_from_its_own_first_hour() {
    let measured = Arc::new(MockLiquidityMoveRepo {
        since: Some(at_october(20, 0)),
        ..MockLiquidityMoveRepo::default()
    });
    let unmeasured = Arc::new(MockLiquidityMoveRepo::default());

    let day = service(&measured)
        .get_liquidity_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let not_yet = service(&unmeasured)
        .get_liquidity_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(day.period.unwrap().covered_since, at_october(20, 0));
    assert!(reads(&measured).is_empty(), "{:?}", reads(&measured));
    assert_eq!(
        not_yet.period.unwrap().covered_since,
        not_yet.period.unwrap().as_of
    );
    assert!(reads(&unmeasured).is_empty());
}

/// Mutation: a pool's moves read over the facts' coverage — read before its liquidity was
/// measured.
#[tokio::test]
async fn a_pools_moves_wait_for_its_protocols_liquidity() {
    let pool = make_pool(pk(1), pk(2), pk(3));
    let early = Arc::new(MockLiquidityMoveRepo {
        since: Some(at_october(13, 13)),
        ..MockLiquidityMoveRepo::default()
    });
    let late = Arc::new(MockLiquidityMoveRepo {
        since: Some(at_october(20, 0)),
        ..MockLiquidityMoveRepo::default()
    });

    service(&early)
        .list_pool_liquidity_moves(Window::Day, &pool, LiquidityMoveOrder::Amount, None, 5)
        .await
        .unwrap();
    service(&late)
        .list_pool_liquidity_moves(Window::Day, &pool, LiquidityMoveOrder::Amount, None, 5)
        .await
        .unwrap();

    assert_eq!(reads(&early), vec!["pool_moves"]);
    assert!(reads(&late).is_empty());
}

/// Mutation: the cursor's instant, window or pool not compared — a page follows one read
/// before a freeze, over another window or of another pool.
#[tokio::test]
async fn a_cursor_of_another_list_or_before_the_last_freeze_is_refused() {
    let pool = make_pool(pk(1), pk(2), pk(3));
    let repo = Arc::new(MockLiquidityMoveRepo {
        since: Some(at_october(13, 13)),
        ..MockLiquidityMoveRepo::default()
    });
    let svc = service(&repo);
    let cursor = |as_of, window, address| LiquidityMoveCursor {
        pool: address,
        window,
        as_of,
        order: LiquidityMoveOrder::Amount,
        amount_usd: Decimal::ONE,
        at: at_october(20, 12),
        signature: "sig".to_owned(),
        event_index: 0,
    };
    let read = |c| {
        svc.list_pool_liquidity_moves(Window::Day, &pool, LiquidityMoveOrder::Amount, Some(c), 5)
    };

    let stale = read(cursor(at_october(20, 12), Window::Day, pk(1))).await;
    let other_window = read(cursor(at_october(20, 13), Window::Week, pk(1))).await;
    let other_pool = read(cursor(at_october(20, 13), Window::Day, pk(7))).await;
    let current = read(cursor(at_october(20, 13), Window::Day, pk(1))).await;

    assert!(matches!(stale, Err(PoolMovesError::CursorRefused)));
    assert!(matches!(other_window, Err(PoolMovesError::CursorRefused)));
    assert!(matches!(other_pool, Err(PoolMovesError::CursorRefused)));
    assert!(current.is_ok());
    assert_eq!(reads(&repo), vec!["pool_moves"]);
}
