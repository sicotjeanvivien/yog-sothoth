//! Metrics emitted by the pool-account worker: an account refused, and a field lost from an
//! account still written.

use metrics::{counter, describe_counter};
use yog_core::application::PoolAccountRejection;
use yog_core::domain::{PoolAccountDegradation, Protocol};

const REJECTED_TOTAL: &str = "yog_context_pool_account_rejected_total";
const DEGRADED_TOTAL: &str = "yog_context_pool_account_degraded_total";

const REJECTED_REASONS: [&str; 4] = [
    "unknown_program",
    "not_a_pool_account",
    "truncated",
    PROTOCOL_MISMATCH,
];
const PROTOCOL_MISMATCH: &str = "protocol_mismatch";
const DEGRADED_REASONS: [&str; 3] = [
    "base_fee_mode_unknown",
    "account_too_short",
    FEE_SCHEDULER_OUT_OF_RANGE,
];
const FEE_SCHEDULER_OUT_OF_RANGE: &str = "fee_scheduler_out_of_range";

pub(crate) struct PoolAccountWorkerMetrics;

impl PoolAccountWorkerMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            REJECTED_TOTAL,
            "Pool account reads refused, so the pool is not written and is read again next \
             cycle: reads, not pools. Labels: protocol (the queue's), \
             reason=unknown_program|not_a_pool_account|truncated|protocol_mismatch"
        );
        describe_counter!(
            DEGRADED_TOTAL,
            "Pool account reads written without a field. Labels: protocol, \
             reason=base_fee_mode_unknown|account_too_short|fee_scheduler_out_of_range"
        );

        // ⚠️ Published at zero: a counter never incremented is not exported, and "flat at 0" would
        // read as absent.
        for protocol in Protocol::all() {
            for reason in REJECTED_REASONS {
                counter!(REJECTED_TOTAL, "protocol" => protocol.as_str(), "reason" => reason)
                    .absolute(0);
            }
            for reason in DEGRADED_REASONS {
                counter!(DEGRADED_TOTAL, "protocol" => protocol.as_str(), "reason" => reason)
                    .absolute(0);
            }
        }
    }

    pub(crate) fn record_rejected(protocol: Protocol, rejection: &PoolAccountRejection) {
        counter!(
            REJECTED_TOTAL,
            "protocol" => protocol.as_str(),
            "reason" => rejected_reason(rejection),
        )
        .increment(1);
    }

    /// An account decoded as another protocol than its queue's.
    pub(crate) fn record_protocol_mismatch(protocol: Protocol) {
        counter!(
            REJECTED_TOTAL,
            "protocol" => protocol.as_str(),
            "reason" => PROTOCOL_MISMATCH,
        )
        .increment(1);
    }

    pub(crate) fn record_degraded(protocol: Protocol, degradation: &PoolAccountDegradation) {
        counter!(
            DEGRADED_TOTAL,
            "protocol" => protocol.as_str(),
            "reason" => degraded_reason(degradation),
        )
        .increment(1);
    }

    /// A curve the store could not hold, reported by the repository.
    pub(crate) fn record_fee_scheduler_out_of_range(protocol: Protocol) {
        counter!(
            DEGRADED_TOTAL,
            "protocol" => protocol.as_str(),
            "reason" => FEE_SCHEDULER_OUT_OF_RANGE,
        )
        .increment(1);
    }
}

fn rejected_reason(rejection: &PoolAccountRejection) -> &'static str {
    match rejection {
        PoolAccountRejection::UnknownProgram { .. } => "unknown_program",
        PoolAccountRejection::NotAPoolAccount { .. } => "not_a_pool_account",
        PoolAccountRejection::Truncated { .. } => "truncated",
    }
}

fn degraded_reason(degradation: &PoolAccountDegradation) -> &'static str {
    match degradation {
        PoolAccountDegradation::BaseFeeModeUnknown => "base_fee_mode_unknown",
        PoolAccountDegradation::AccountTooShort { .. } => "account_too_short",
    }
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
