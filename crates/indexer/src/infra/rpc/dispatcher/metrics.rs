use metrics::{counter, describe_counter};
use yog_core::domain::Protocol;

/// Total de [`RawLogEvent`](super::types::RawLogEvent) reçus du Listener.
const EVENTS_RECEIVED: &str = "yog_indexer_raw_log_events_total";

/// Événements rejetés par un filtre.
const EVENTS_REJECTED: &str = "yog_indexer_raw_log_events_rejected_total";

/// Événements dont la signature brute n'a pas pu être parsée en `Signature`.
const EVENTS_MALFORMED: &str = "yog_indexer_raw_log_events_malformed_total";

/// Signatures qualifiées émises vers l'Indexer.
const SIGNATURES_EMITTED: &str = "yog_indexer_qualified_signatures_total";

/// Signatures droppées parce que le channel aval (Indexer) est saturé.
const DOWNSTREAM_SATURATED: &str = "yog_indexer_downstream_saturated_total";

pub struct DispatcherMetrics;

impl DispatcherMetrics {
    /// À appeler une fois au démarrage pour enregistrer les descriptions
    /// auprès de l'exporter Prometheus.
    pub fn register_descriptions() {
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

    pub fn record_received(protocol: &Protocol) {
        counter!(EVENTS_RECEIVED, "protocol" => protocol.as_str()).increment(1);
    }

    pub fn record_rejected(protocol: &Protocol, filter_name: &'static str, reason: &'static str) {
        counter!(
            EVENTS_REJECTED,
            "protocol" => protocol.as_str(),
            "filter"   => filter_name,
            "reason"   => reason,
        )
        .increment(1);
    }

    pub fn record_malformed(protocol: &Protocol) {
        counter!(EVENTS_MALFORMED, "protocol" => protocol.as_str()).increment(1);
    }

    pub fn record_emitted(protocol: &Protocol) {
        counter!(SIGNATURES_EMITTED, "protocol" => protocol.as_str()).increment(1);
    }

    pub fn record_downstream_saturated(protocol: &Protocol) {
        counter!(DOWNSTREAM_SATURATED, "protocol" => protocol.as_str()).increment(1);
    }
}
