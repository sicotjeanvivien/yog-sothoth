use rust_decimal::Decimal;

use super::RunSplit;

fn split(volume: Option<i64>, in_runs: Option<i64>) -> RunSplit {
    RunSplit {
        volume_usd: volume.map(Decimal::from),
        in_runs_usd: in_runs.map(Decimal::from),
    }
}

/// Mutation: the floor dropped — runs worth more than the bar's valued volume served a negative
/// rest.
#[test]
fn the_volume_outside_the_runs_is_never_under_zero() {
    assert_eq!(
        split(Some(100), Some(30)).outside_runs_usd(),
        Some(Decimal::from(70))
    );
    assert_eq!(
        split(Some(100), Some(130)).outside_runs_usd(),
        Some(Decimal::ZERO)
    );
}

/// Mutation: an unknown term read as 0 — a bar the runs do not cover served wholly outside them.
#[test]
fn the_volume_outside_the_runs_is_unknown_as_soon_as_one_term_is() {
    assert_eq!(split(Some(100), None).outside_runs_usd(), None);
    assert_eq!(split(None, Some(30)).outside_runs_usd(), None);
}
