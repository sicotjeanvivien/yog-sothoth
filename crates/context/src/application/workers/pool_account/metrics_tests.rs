use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use solana_pubkey::Pubkey;
use yog_core::application::PoolAccountRejection;
use yog_core::domain::{PoolAccountDegradation, Protocol};

use super::*;

/// Mutation: two variants swapped onto each other's label, or a label the README does not list.
#[test]
fn each_rejection_has_its_own_label() {
    let cases = [
        (
            PoolAccountRejection::UnknownProgram {
                program_id: Pubkey::new_unique(),
            },
            "unknown_program",
        ),
        (
            PoolAccountRejection::NotAPoolAccount {
                protocol: Protocol::MeteoraDammV2,
            },
            "not_a_pool_account",
        ),
        (
            PoolAccountRejection::Truncated {
                protocol: Protocol::MeteoraDammV2,
                len: 1,
                min: 2,
            },
            "truncated",
        ),
    ];
    for (rejection, label) in cases {
        assert_eq!(rejected_reason(&rejection), label);
        assert!(
            REJECTED_REASONS.contains(&label),
            "{label} is published at 0"
        );
    }
    assert!(REJECTED_REASONS.contains(&PROTOCOL_MISMATCH));
}

/// Mutation: the two degradations swapped, or a label missing from the zero list.
#[test]
fn each_degradation_has_its_own_label() {
    let cases = [
        (
            PoolAccountDegradation::BaseFeeModeUnknown,
            "base_fee_mode_unknown",
        ),
        (
            PoolAccountDegradation::AccountTooShort { len: 1, needed: 2 },
            "account_too_short",
        ),
    ];
    for (degradation, label) in cases {
        assert_eq!(degraded_reason(&degradation), label);
        assert!(
            DEGRADED_REASONS.contains(&label),
            "{label} is published at 0"
        );
    }
    assert!(DEGRADED_REASONS.contains(&FEE_SCHEDULER_OUT_OF_RANGE));
}

/// Mutation: a family, a protocol or a reason left out of the zero publication.
#[test]
fn every_series_is_published_at_zero_before_any_cycle() {
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();
    ::metrics::with_local_recorder(&recorder, PoolAccountWorkerMetrics::register_descriptions);
    let snapshot = snapshotter.snapshot().into_vec();

    let families = [
        (REJECTED_TOTAL, &REJECTED_REASONS[..]),
        (DEGRADED_TOTAL, &DEGRADED_REASONS[..]),
    ];
    for (name, reasons) in families {
        for protocol in Protocol::all() {
            for reason in reasons {
                let value = snapshot.iter().find_map(|(key, _, _, v)| {
                    let labels: Vec<_> = key.key().labels().collect();
                    (key.key().name() == name
                        && labels
                            .iter()
                            .any(|l| l.key() == "protocol" && l.value() == protocol.as_str())
                        && labels
                            .iter()
                            .any(|l| l.key() == "reason" && l.value() == *reason))
                    .then_some(v)
                });
                assert_eq!(
                    value,
                    Some(&DebugValue::Counter(0)),
                    "{name}{{{protocol}, {reason}}}"
                );
            }
        }
    }
}
