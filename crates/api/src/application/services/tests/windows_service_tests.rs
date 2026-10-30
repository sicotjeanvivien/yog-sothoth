//! Unit tests for `WindowsService`: every window, read on its own kind of period.

use yog_core::domain::{Window, WrittenTranches};

use super::{ReadPeriod, WindowsService};
use crate::testing::{MockProtocolActivityRepo, at_october, frozen_hours_repo, make_frozen_hours};

/// Mutation: the last hour read on the frozen hours, which would close it with the others, or a
/// window left out.
#[tokio::test]
async fn every_window_reads_its_own_period_the_shortest_first() {
    let svc = WindowsService::new(std::sync::Arc::new(
        MockProtocolActivityRepo::with_hours_and_tranches(
            make_frozen_hours(),
            WrittenTranches {
                first: at_october(13, 13),
                through: at_october(20, 12),
            },
        ),
    ));

    let windows = svc.list().await.unwrap();

    let names: Vec<_> = windows.iter().map(|(window, _)| *window).collect();
    assert_eq!(
        names,
        [Window::Hour, Window::Day, Window::Week, Window::Month]
    );
    assert!(matches!(windows[0].1, Some(ReadPeriod::Tranche(_))));
    assert!(
        windows[1..]
            .iter()
            .all(|w| matches!(w.1, Some(ReadPeriod::Frozen(_))))
    );
}

/// Mutation: a read of a window without facts failing, where it says only that it is not open.
#[tokio::test]
async fn before_any_fact_every_window_is_listed_without_a_period() {
    let svc = WindowsService::new(frozen_hours_repo(None));

    let windows = svc.list().await.unwrap();

    assert_eq!(windows.len(), 4);
    assert!(windows.iter().all(|(_, period)| period.is_none()));
}
