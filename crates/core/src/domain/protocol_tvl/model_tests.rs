use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;

use super::{ProtocolTvlChange, TvlReading};
use crate::domain::Protocol;

fn reading(tvl: Option<i64>) -> TvlReading {
    TvlReading {
        at: Utc.with_ymd_and_hms(2026, 10, 25, 3, 0, 0).unwrap(),
        tvl_usd: tvl.map(Decimal::from),
    }
}

fn change(start: Option<TvlReading>, discovered: Option<i64>) -> ProtocolTvlChange {
    ProtocolTvlChange {
        protocol: Protocol::MeteoraDammV2,
        end: reading(Some(1_300)),
        start,
        discovered_tvl_usd: discovered.map(Decimal::from),
    }
}

/// Mutation: the discovered part left in the change — a pool seen for the first time read as a
/// deposit, 1300 − 1000 = 300 instead of 100.
#[test]
fn the_change_leaves_out_the_discovered_part() {
    assert_eq!(
        change(Some(reading(Some(1_000))), Some(200)).change_usd(),
        Some(Decimal::from(100))
    );
}

/// Mutation: an unknown term read as 0 — a part not measured, or a start not read, taken for a
/// change.
#[test]
fn the_change_is_unknown_as_soon_as_one_term_is() {
    assert_eq!(change(Some(reading(Some(1_000))), None).change_usd(), None);
    assert_eq!(change(None, Some(200)).change_usd(), None);
    assert_eq!(change(Some(reading(None)), Some(200)).change_usd(), None);
}

/// Mutation: a last reading given a start or a discovered part — a change published over an hour
/// that has no discovered part.
#[test]
fn a_last_reading_alone_has_no_change() {
    let at = Utc.with_ymd_and_hms(2026, 10, 25, 3, 0, 0).unwrap();
    let last = ProtocolTvlChange::last_reading(super::ProtocolTvl {
        protocol: Protocol::MeteoraDammV2,
        at,
        tvl_usd: Some(Decimal::from(5)),
        pools_priced: 1,
        pools_unvalued: 0,
    });

    assert_eq!(last.end, reading(Some(5)));
    assert_eq!(last.start, None);
    assert_eq!(last.discovered_tvl_usd, None);
    assert_eq!(last.change_usd(), None);
}

fn tvl_reading(protocol: Protocol, tvl: Option<i64>) -> super::ProtocolTvl {
    super::ProtocolTvl {
        protocol,
        at: Utc.with_ymd_and_hms(2026, 10, 25, 3, 0, 0).unwrap(),
        tvl_usd: tvl.map(Decimal::from),
        pools_priced: 1,
        pools_unvalued: 0,
    }
}

fn swaps(protocol: Protocol, volume: Option<i64>) -> crate::domain::ProtocolSwapActivity {
    crate::domain::ProtocolSwapActivity {
        protocol,
        activity: crate::domain::SwapActivity {
            swaps: 1,
            volume_usd: volume.map(Decimal::from),
            fees_usd: None,
            lp_fees_usd: None,
            protocol_fees_usd: None,
            referral_fees_usd: None,
            pool_hours: 10,
            pool_hours_valued: 9,
        },
        pools: None,
    }
}

/// Mutation: a field left at one row's, or a start unknown skipped — a total change over a start
/// smaller than the truth.
#[test]
fn a_total_adds_every_protocol_and_is_unknown_with_one_term() {
    let whole = |protocol, end: i64, start: i64| super::ProtocolTvlChange {
        protocol,
        end: reading(Some(end)),
        start: Some(reading(Some(start))),
        discovered_tvl_usd: Some(Decimal::ONE),
    };

    let total = super::ProtocolTvlChange::total(&[
        whole(Protocol::MeteoraDammV2, 100, 90),
        whole(Protocol::MeteoraDlmm, 50, 40),
    ])
    .unwrap();
    assert_eq!(total.end_tvl_usd, Some(Decimal::from(150)));
    assert_eq!(total.start_tvl_usd, Some(Decimal::from(130)));
    assert_eq!(total.discovered_tvl_usd, Some(Decimal::TWO));
    assert_eq!(total.change_usd, Some(Decimal::from(18)));

    let total = super::ProtocolTvlChange::total(&[
        whole(Protocol::MeteoraDammV2, 100, 90),
        change(None, Some(1)),
    ])
    .unwrap();
    assert_eq!(total.start_tvl_usd, None);
    assert_eq!(total.change_usd, None);

    assert_eq!(super::ProtocolTvlChange::total(&[]), None);
}

/// Mutation: the total the sum of the ratios, 5 here — a turnover no protocol has.
#[test]
fn the_total_turnover_is_every_volume_over_every_tvl() {
    let total = super::LiquidityTurnover::total(
        &[
            swaps(Protocol::MeteoraDammV2, Some(100)),
            swaps(Protocol::MeteoraDlmm, Some(50)),
        ],
        &[
            tvl_reading(Protocol::MeteoraDammV2, Some(50)),
            tvl_reading(Protocol::MeteoraDlmm, Some(25)),
        ],
    )
    .unwrap();

    assert_eq!(total.turnover, Some(Decimal::TWO));
    assert_eq!(total.pool_hours, 20);
    assert_eq!(total.pool_hours_valued, 18);
}

/// Mutation: a TVL of 0 divided, a TVL unknown read as 0, or a row given another protocol's TVL.
#[test]
fn a_turnover_is_unknown_without_a_tvl_above_zero() {
    let rows = super::ProtocolLiquidityTurnover::rows(
        &[
            swaps(Protocol::MeteoraDammV2, Some(100)),
            swaps(Protocol::MeteoraDlmm, Some(50)),
        ],
        &[
            tvl_reading(Protocol::MeteoraDlmm, Some(0)),
            tvl_reading(Protocol::MeteoraDammV2, Some(25)),
        ],
    );

    let turnovers: Vec<_> = rows.iter().map(|row| row.turnover.turnover).collect();
    assert_eq!(turnovers, [Some(Decimal::from(4)), None]);
    assert_eq!(
        rows[0].tvl_at,
        Some(tvl_reading(Protocol::MeteoraDammV2, None).at)
    );
    let below = super::ProtocolLiquidityTurnover::rows(
        &[swaps(Protocol::MeteoraDlmm, Some(50))],
        &[tvl_reading(Protocol::MeteoraDlmm, Some(-1))],
    );
    assert_eq!(below[0].turnover.turnover, None);
    let unread =
        super::ProtocolLiquidityTurnover::rows(&[swaps(Protocol::MeteoraDlmm, Some(50))], &[]);
    assert_eq!(unread[0].turnover.turnover, None);
    assert_eq!(unread[0].tvl_at, None);
    assert_eq!(
        super::LiquidityTurnover::total(&[swaps(Protocol::MeteoraDammV2, Some(1))], &[])
            .unwrap()
            .turnover,
        None
    );
}
