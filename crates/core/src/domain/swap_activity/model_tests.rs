use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;

use super::{ProtocolSwapActivity, SwapActivity};
use crate::domain::{Protocol, ProtocolBucket};

fn activity(volume: Option<i64>) -> SwapActivity {
    SwapActivity {
        swaps: 2,
        volume_usd: volume.map(Decimal::from),
        fees_usd: Some(Decimal::ONE),
        lp_fees_usd: None,
        protocol_fees_usd: Some(Decimal::TWO),
        referral_fees_usd: Some(Decimal::ZERO),
        pool_hours: 10,
        pool_hours_valued: 8,
    }
}

fn row(protocol: Protocol, volume: Option<i64>, pools: Option<i64>) -> ProtocolSwapActivity {
    ProtocolSwapActivity {
        protocol,
        activity: activity(volume),
        pools,
    }
}

/// Mutation: a field left at one row's, or a count not added — the total of the first protocol.
#[test]
fn a_total_adds_every_field_of_every_protocol() {
    let total = ProtocolSwapActivity::total(&[
        row(Protocol::MeteoraDammV2, Some(30), Some(3)),
        row(Protocol::MeteoraDlmm, Some(12), Some(4)),
    ])
    .unwrap();

    assert_eq!(total.activity.swaps, 4);
    assert_eq!(total.activity.volume_usd, Some(Decimal::from(42)));
    assert_eq!(total.activity.fees_usd, Some(Decimal::TWO));
    assert_eq!(total.activity.protocol_fees_usd, Some(Decimal::from(4)));
    assert_eq!(total.activity.pool_hours, 20);
    assert_eq!(total.activity.pool_hours_valued, 16);
    assert_eq!(total.pools, Some(7));
}

/// Mutation: a term unknown skipped — a total smaller than the truth, shown as whole.
#[test]
fn one_term_unknown_makes_the_total_unknown() {
    let total = ProtocolSwapActivity::total(&[
        row(Protocol::MeteoraDammV2, Some(30), Some(3)),
        row(Protocol::MeteoraDlmm, None, None),
    ])
    .unwrap();

    assert_eq!(total.activity.volume_usd, None);
    assert_eq!(total.pools, None);
    assert_eq!(total.activity.fees_usd, Some(Decimal::TWO));
}

/// Mutation: no row read as a total of zero — a period not read shown as one without swaps.
#[test]
fn no_row_has_no_total() {
    assert_eq!(ProtocolSwapActivity::total(&[]), None);
}

/// Mutation: a bucket kept per protocol, or one unknown value emptying its bar.
#[test]
fn a_bucket_adds_its_protocols_and_skips_what_one_does_not_know() {
    let at = |hour| Utc.with_ymd_and_hms(2026, 10, 25, hour, 0, 0).unwrap();
    let bucket = |protocol, hour, volume| ProtocolBucket {
        protocol,
        from: at(hour),
        value: activity(volume),
    };

    let buckets = SwapActivity::bucket_totals(&[
        bucket(Protocol::MeteoraDammV2, 4, Some(5)),
        bucket(Protocol::MeteoraDammV2, 3, Some(1)),
        bucket(Protocol::MeteoraDlmm, 3, None),
        bucket(Protocol::MeteoraDlmm, 4, Some(7)),
        bucket(Protocol::MeteoraDlmm, 5, None),
    ]);

    let drawn: Vec<_> = buckets
        .iter()
        .map(|b| (b.from, b.activity.volume_usd))
        .collect();
    assert_eq!(
        drawn,
        [
            (at(3), Some(Decimal::ONE)),
            (at(4), Some(Decimal::from(12))),
            (at(5), None),
        ]
    );
    assert_eq!(buckets[0].activity.pool_hours, 20);
    assert_eq!(buckets[0].activity.pool_hours_valued, 16);
    assert_eq!(buckets[0].activity.swaps, 4);
}

/// Mutation: an unchecked add — a mispriced token panics the request instead of being unknown.
#[test]
fn a_sum_that_overflows_is_unknown() {
    let huge = |protocol| ProtocolSwapActivity {
        protocol,
        activity: SwapActivity {
            volume_usd: Some(Decimal::MAX),
            ..activity(None)
        },
        pools: None,
    };

    let total =
        ProtocolSwapActivity::total(&[huge(Protocol::MeteoraDammV2), huge(Protocol::MeteoraDlmm)])
            .unwrap();
    assert_eq!(total.activity.volume_usd, None);

    let at = Utc.with_ymd_and_hms(2026, 10, 25, 3, 0, 0).unwrap();
    let buckets = SwapActivity::bucket_totals(
        &[Protocol::MeteoraDammV2, Protocol::MeteoraDlmm].map(|protocol| ProtocolBucket {
            protocol,
            from: at,
            value: huge(protocol).activity,
        }),
    );
    assert_eq!(buckets[0].activity.volume_usd, None);

    let mut rows = vec![
        ProtocolBucket {
            protocol: Protocol::MeteoraDammV2,
            from: at,
            value: huge(Protocol::MeteoraDammV2).activity,
        };
        2
    ];
    rows.push(ProtocolBucket {
        protocol: Protocol::MeteoraDlmm,
        from: at,
        value: activity(Some(1)),
    });
    assert_eq!(
        SwapActivity::bucket_totals(&rows)[0].activity.volume_usd,
        None
    );
}
