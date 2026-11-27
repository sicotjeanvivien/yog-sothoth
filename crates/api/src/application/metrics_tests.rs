use yog_core::domain::{FeeSchedulerDrop, FieldOutOfRange, Protocol};

use super::*;
use crate::testing::{counter, recorded};

/// Mutation: the two drops swapped onto each other's label, or a label left out of the zero list.
#[test]
fn each_fee_curve_drop_has_its_own_label() {
    let cases = [
        (
            FeeSchedulerDrop::OutOfRange(FieldOutOfRange { field: "x" }),
            "fee_scheduler_out_of_range",
        ),
        (FeeSchedulerDrop::PartlyNull, "fee_scheduler_partly_null"),
    ];
    for (drop, label) in cases {
        assert_eq!(degraded_reason(drop), label);
        assert!(
            POOL_PROPERTIES_DEGRADED_REASONS.contains(&label),
            "{label} is published at 0"
        );
    }
}

/// Mutation: a protocol or a reason left out of the zero publication.
#[test]
fn every_degraded_series_is_published_at_zero_at_startup() {
    let (_, counters) = recorded(async { ApiMetrics::register_descriptions() });

    for protocol in Protocol::all() {
        for reason in POOL_PROPERTIES_DEGRADED_REASONS {
            assert_eq!(
                counter(
                    &counters,
                    POOL_PROPERTIES_DEGRADED,
                    &[("protocol", protocol.as_str()), ("reason", reason)]
                ),
                Some(0),
                "{protocol}, {reason}"
            );
        }
    }
}
