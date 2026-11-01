//! Unit tests for `LiquidityTurnoverService`: what each of its reads covers.

use std::sync::Arc;

use rust_decimal::Decimal;
use yog_core::domain::{Protocol, ProtocolSwapActivity, ProtocolTvl, SwapActivity, Window};

use super::LiquidityTurnoverService;
use crate::testing::{
    MockProtocolActivityRepo, MockProtocolTvlRepo, MockSwapActivityRepo, at_october,
    frozen_hours_repo, make_frozen_hours, tranches_begun,
};

/// Mutation: a TVL read, or a volume, for a window the facts do not cover or that has no row.
#[tokio::test]
async fn a_window_without_a_row_reads_no_tvl() {
    let swaps = Arc::new(MockSwapActivityRepo::default());
    let tvl = Arc::new(MockProtocolTvlRepo::default());
    let svc = LiquidityTurnoverService::new(
        frozen_hours_repo(Some(make_frozen_hours())),
        swaps.clone(),
        tvl.clone(),
    );

    let month = svc.get_totals(Window::Month, None).await.unwrap();
    assert!(month.measured.items.is_empty());
    assert_eq!(month.total, None);
    assert!(swaps.reads.lock().unwrap().is_empty());

    let week = svc
        .get_totals(Window::Week, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    assert_eq!(swaps.reads.lock().unwrap().len(), 1);
    assert!(week.measured.items.is_empty());
    assert!(tvl.instants.lock().unwrap().is_empty());
}

fn swaps_of_200_usd() -> Vec<ProtocolSwapActivity> {
    vec![ProtocolSwapActivity {
        protocol: Protocol::MeteoraDammV2,
        activity: SwapActivity {
            swaps: 1,
            volume_usd: Some(Decimal::from(200)),
            fees_usd: None,
            lp_fees_usd: None,
            protocol_fees_usd: None,
            referral_fees_usd: None,
            pool_hours: 1,
            pool_hours_valued: 1,
        },
        pools: Some(1),
    }]
}

fn tvl_of_100_usd() -> Vec<ProtocolTvl> {
    vec![ProtocolTvl {
        protocol: Protocol::MeteoraDammV2,
        at: at_october(20, 13),
        tvl_usd: Some(Decimal::from(100)),
        pools_priced: 1,
        pools_unvalued: 0,
    }]
}

/// Mutation: the TVL never read, or read at another hour — a ratio absent or over another stock.
#[tokio::test]
async fn the_ratio_is_the_volume_over_the_last_frozen_tvl() {
    let swaps = Arc::new(MockSwapActivityRepo {
        totals: swaps_of_200_usd(),
        ..MockSwapActivityRepo::default()
    });
    let tvl = Arc::new(MockProtocolTvlRepo {
        latest: tvl_of_100_usd(),
        ..MockProtocolTvlRepo::default()
    });
    let hours = Arc::new(MockProtocolActivityRepo::with_hours_and_tranches(
        make_frozen_hours(),
        tranches_begun(120),
    ));
    let svc = LiquidityTurnoverService::new(hours, swaps.clone(), tvl.clone());

    for window in [Window::Week, Window::Hour] {
        let read = svc.get_totals(window, None).await.unwrap();
        assert_eq!(read.total.unwrap().turnover, Some(Decimal::from(2)));
        assert_eq!(
            read.measured.items[0].turnover.turnover,
            Some(Decimal::from(2))
        );
    }

    assert_eq!(
        *tvl.instants.lock().unwrap(),
        [
            ("tvl", at_october(20, 12), None),
            ("tvl", at_october(20, 12), None)
        ]
    );
    assert_eq!(swaps.tranche_reads.lock().unwrap().len(), 1);
}

/// Mutation: the TVL read for every protocol — one protocol's volume over all their stock.
#[tokio::test]
async fn a_protocols_ratio_reads_its_own_tvl() {
    let tvl = Arc::new(MockProtocolTvlRepo {
        latest: tvl_of_100_usd(),
        ..MockProtocolTvlRepo::default()
    });
    let swaps = Arc::new(MockSwapActivityRepo {
        totals: swaps_of_200_usd(),
        ..MockSwapActivityRepo::default()
    });
    let svc = LiquidityTurnoverService::new(
        frozen_hours_repo(Some(make_frozen_hours())),
        swaps,
        tvl.clone(),
    );

    svc.get_totals(Window::Week, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert_eq!(
        *tvl.instants.lock().unwrap(),
        [("tvl", at_october(20, 12), Some(Protocol::MeteoraDammV2))]
    );
}

/// Mutation: the hour's guard forced to true — a ratio over half an hour's tranches.
#[tokio::test]
async fn an_hour_the_tranches_do_not_cover_has_no_ratio() {
    let swaps = Arc::new(MockSwapActivityRepo {
        totals: swaps_of_200_usd(),
        ..MockSwapActivityRepo::default()
    });
    let tvl = Arc::new(MockProtocolTvlRepo {
        latest: tvl_of_100_usd(),
        ..MockProtocolTvlRepo::default()
    });
    let hours = Arc::new(MockProtocolActivityRepo::with_hours_and_tranches(
        make_frozen_hours(),
        tranches_begun(30),
    ));
    let svc = LiquidityTurnoverService::new(hours, swaps.clone(), tvl.clone());

    let read = svc.get_totals(Window::Hour, None).await.unwrap();

    assert_eq!(read.total, None);
    assert!(swaps.tranche_reads.lock().unwrap().is_empty());
    assert!(tvl.instants.lock().unwrap().is_empty());
}
