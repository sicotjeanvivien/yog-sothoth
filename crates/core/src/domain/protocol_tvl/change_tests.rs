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
        protocol_part: None,
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
        protocol_part: None,
    });

    assert_eq!(last.end, reading(Some(5)));
    assert_eq!(last.start, None);
    assert_eq!(last.discovered_tvl_usd, None);
    assert_eq!(last.change_usd(), None);
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
        protocol_part: None,
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
