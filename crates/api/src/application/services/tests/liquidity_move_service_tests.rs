//! Unit tests for `LiquidityMoveService`: the liquidity's own coverage, the reads a period it
//! does not cover never makes, and the cursors a page refuses.

use std::sync::Arc;

use rust_decimal::Decimal;
use yog_core::domain::{
    LiquidityMoveCursor, LiquidityMoveList, LiquidityMoveOrder, LiquidityMoveScope, Protocol,
    Window, WrittenTranches,
};

use super::{LiquidityMoveService, MovesError};
use crate::application::services::period::PeriodBounds;
use crate::testing::{
    MockLiquidityMoveRepo, MockProtocolActivityRepo, at_october, frozen_hours_repo,
    make_frozen_hours, make_pool, pk,
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

    assert_eq!(day.period.unwrap().covered_since(), at_october(20, 0));
    assert!(reads(&measured).is_empty(), "{:?}", reads(&measured));
    assert_eq!(
        not_yet.period.unwrap().covered_since(),
        not_yet.period.unwrap().as_of()
    );
    assert!(reads(&unmeasured).is_empty());
}

/// Mutation: the moves read over the facts' coverage — a protocol's or a pool's read before
/// its liquidity was measured.
#[tokio::test]
async fn the_moves_wait_for_their_protocols_liquidity() {
    let pool = make_pool(pk(1), pk(2), pk(3));
    for scope in [
        LiquidityMoveScope::Pool(pool.pool_address),
        LiquidityMoveScope::Protocol(pool.protocol),
    ] {
        let early = Arc::new(MockLiquidityMoveRepo {
            since: Some(at_october(13, 13)),
            ..MockLiquidityMoveRepo::default()
        });
        let late = Arc::new(MockLiquidityMoveRepo {
            since: Some(at_october(20, 0)),
            ..MockLiquidityMoveRepo::default()
        });

        for repo in [&early, &late] {
            service(repo)
                .list_liquidity_moves(
                    Window::Day,
                    LiquidityMoveList::every_kind(scope),
                    pool.protocol,
                    LiquidityMoveOrder::Amount,
                    None,
                    5,
                )
                .await
                .unwrap();
        }

        assert_eq!(reads(&early), vec!["moves"], "{scope:?}");
        assert!(reads(&late).is_empty(), "{scope:?}");
    }
}

/// Mutation: the cursor's end, its step or its window not compared, or the page read over the
/// latest end — a page follows one read two freezes back, ahead, mid-hour, or skips an hour.
#[tokio::test]
async fn a_cursor_is_read_over_its_own_end_within_the_margin() {
    let pool = make_pool(pk(1), pk(2), pk(3));
    let repo = Arc::new(MockLiquidityMoveRepo {
        since: Some(at_october(13, 13)),
        ..MockLiquidityMoveRepo::default()
    });
    let svc = service(&repo);
    let cursor = |as_of, window, address| LiquidityMoveCursor {
        list: LiquidityMoveList::every_kind(LiquidityMoveScope::Pool(address)),
        window,
        as_of,
        order: LiquidityMoveOrder::Amount,
        amount_usd: Decimal::ONE,
        at: at_october(20, 12),
        signature: "sig".to_owned(),
        event_index: 0,
    };
    let read = |c| {
        svc.list_liquidity_moves(
            Window::Day,
            LiquidityMoveList::every_kind(LiquidityMoveScope::Pool(pool.pool_address)),
            pool.protocol,
            LiquidityMoveOrder::Amount,
            Some(c),
            5,
        )
    };

    let stale = read(cursor(at_october(20, 11), Window::Day, pk(1))).await;
    let ahead = read(cursor(at_october(20, 14), Window::Day, pk(1))).await;
    let other_window = read(cursor(at_october(20, 13), Window::Week, pk(1))).await;
    let off_step = read(cursor(
        at_october(20, 13) - chrono::TimeDelta::minutes(30),
        Window::Day,
        pk(1),
    ))
    .await;
    let behind = read(cursor(at_october(20, 12), Window::Day, pk(1))).await;
    let current = read(cursor(at_october(20, 13), Window::Day, pk(1))).await;

    assert!(matches!(stale, Err(MovesError::CursorRefused)));
    assert!(matches!(ahead, Err(MovesError::CursorRefused)));
    assert!(matches!(other_window, Err(MovesError::CursorRefused)));
    assert!(matches!(off_step, Err(MovesError::CursorRefused)));
    assert_eq!(behind.unwrap().period.unwrap().as_of(), at_october(20, 12));
    assert!(current.is_ok());
    let ends: Vec<_> = repo
        .reads
        .lock()
        .unwrap()
        .iter()
        .map(|r| (r.1.from, r.1.as_of))
        .collect();
    assert_eq!(
        ends,
        vec![
            (at_october(19, 12), at_october(20, 12)),
            (at_october(19, 13), at_october(20, 13))
        ]
    );
}

/// Mutation: the last hour covered from the tranches held alone — read while the liquidity is
/// measured from inside it.
#[tokio::test]
async fn the_last_hour_is_covered_from_the_later_of_the_tranches_and_the_liquidity() {
    let tranches = WrittenTranches {
        first: at_october(13, 13),
        through: at_october(20, 12) + chrono::TimeDelta::minutes(40),
    };
    let in_hour = |since| {
        let repo = Arc::new(MockLiquidityMoveRepo {
            since: Some(since),
            ..MockLiquidityMoveRepo::default()
        });
        let hours = Arc::new(MockProtocolActivityRepo::with_tranches(tranches));
        (LiquidityMoveService::new(hours, repo.clone()), repo)
    };
    let (late, late_repo) = in_hour(tranches.through - chrono::TimeDelta::minutes(30));
    let (early, early_repo) = in_hour(at_october(14, 0));

    let late = late
        .get_liquidity_totals(Window::Hour, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    early
        .get_liquidity_totals(Window::Hour, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(
        late.period.unwrap().covered_since(),
        tranches.through - chrono::TimeDelta::minutes(30)
    );
    assert!(late.items.is_empty() && late_repo.tranche_reads.lock().unwrap().is_empty());
    assert_eq!(early_repo.tranche_reads.lock().unwrap().len(), 1);
}

/// Mutation: the last hour's moves read as a frozen period, or a later page over the latest end —
/// a page of the hour read on the hours, or skipping the tranche written since the first.
#[tokio::test]
async fn the_last_hours_moves_are_read_on_its_tranches() {
    let tranches = WrittenTranches {
        first: at_october(13, 13),
        through: at_october(20, 12) + chrono::TimeDelta::minutes(40),
    };
    let repo = Arc::new(MockLiquidityMoveRepo {
        since: Some(at_october(14, 0)),
        ..MockLiquidityMoveRepo::default()
    });
    let hours = Arc::new(MockProtocolActivityRepo::with_tranches(tranches));
    let svc = LiquidityMoveService::new(hours, repo.clone());
    let list = LiquidityMoveList::every_kind(LiquidityMoveScope::Protocol(Protocol::MeteoraDammV2));
    let read = |cursor| {
        svc.list_liquidity_moves(
            Window::Hour,
            list,
            Protocol::MeteoraDammV2,
            LiquidityMoveOrder::Amount,
            cursor,
            5,
        )
    };
    let cursor = |as_of| LiquidityMoveCursor {
        list,
        window: Window::Hour,
        as_of,
        order: LiquidityMoveOrder::Amount,
        amount_usd: Decimal::ONE,
        at: tranches.through - chrono::TimeDelta::minutes(10),
        signature: "sig".to_owned(),
        event_index: 0,
    };

    let minutes = chrono::TimeDelta::minutes;
    let first = read(None).await.unwrap();
    let moved = read(Some(cursor(tranches.through - minutes(5)))).await;
    let stale = read(Some(cursor(tranches.through - minutes(65)))).await;
    let off_step = read(Some(cursor(tranches.through - minutes(2)))).await;

    let period = first.period.unwrap();
    assert_eq!(
        (period.from(), period.as_of()),
        (tranches.through - minutes(60), tranches.through)
    );
    assert_eq!(
        moved.unwrap().period.unwrap().as_of(),
        tranches.through - minutes(5)
    );
    assert!(matches!(stale, Err(MovesError::CursorRefused)));
    assert!(matches!(off_step, Err(MovesError::CursorRefused)));
    let tranche_reads: Vec<_> = repo
        .tranche_reads
        .lock()
        .unwrap()
        .iter()
        .map(|r| (r.0, r.1.from, r.1.as_of))
        .collect();
    assert_eq!(
        tranche_reads,
        vec![
            (
                "moves_in_tranches",
                tranches.through - minutes(60),
                tranches.through
            ),
            (
                "moves_in_tranches",
                tranches.through - minutes(65),
                tranches.through - minutes(5)
            ),
        ]
    );
    assert!(reads(&repo).is_empty(), "{:?}", reads(&repo));
}
