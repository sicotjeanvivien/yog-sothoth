use metrics::{counter, describe_counter};
use yog_core::domain::Protocol;

/// Total [`RawLogEvent`](super::types::RawLogEvent) received from the listener.
const EVENTS_RECEIVED: &str = "yog_indexer_raw_log_events_total";

/// Events rejected by a filter.
const EVENTS_REJECTED: &str = "yog_indexer_raw_log_events_rejected_total";

/// Events whose raw signature failed to parse into a `Signature`.
const EVENTS_MALFORMED: &str = "yog_indexer_raw_log_events_malformed_total";

/// Qualified signatures emitted towards the indexer.
const SIGNATURES_EMITTED: &str = "yog_indexer_qualified_signatures_total";

/// Signatures dropped because the downstream channel (indexer) is saturated.
const DOWNSTREAM_SATURATED: &str = "yog_indexer_downstream_saturated_total";

pub struct DispatcherMetrics;

impl DispatcherMetrics {
    /// Call once at startup to register the descriptions with the
    /// Prometheus exporter.
    pub(crate) fn register_descriptions() {
        describe_counter!(EVENTS_RECEIVED, "Raw log events received from the listener");
        describe_counter!(EVENTS_REJECTED, "Raw log events rejected by a filter");
        describe_counter!(
            EVENTS_MALFORMED,
            "Raw log events with a signature that failed to parse"
        );
        describe_counter!(
            SIGNATURES_EMITTED,
            "Qualified signatures emitted to the indexer"
        );
        describe_counter!(
            DOWNSTREAM_SATURATED,
            "Qualified signatures dropped because the indexer channel was full"
        );
    }

    pub(crate) fn record_received(protocol: &Protocol) {
        counter!(EVENTS_RECEIVED, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_rejected(
        protocol: &Protocol,
        filter_name: &'static str,
        reason: &'static str,
    ) {
        counter!(
            EVENTS_REJECTED,
            "protocol" => protocol.as_str(),
            "filter"   => filter_name,
            "reason"   => reason,
        )
        .increment(1);
    }

    pub(crate) fn record_malformed(protocol: &Protocol) {
        counter!(EVENTS_MALFORMED, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_emitted(protocol: &Protocol) {
        counter!(SIGNATURES_EMITTED, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_downstream_saturated(protocol: &Protocol) {
        counter!(DOWNSTREAM_SATURATED, "protocol" => protocol.as_str()).increment(1);
    }
}
