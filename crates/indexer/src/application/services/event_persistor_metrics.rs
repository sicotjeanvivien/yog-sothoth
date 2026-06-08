//! Metrics emitted by the EventPersistor.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const INSTRUCTIONS_INDEXED: &str = "yog_indexer_instructions_indexed_total";
const PERSIST_DURATION: &str = "yog_indexer_persist_duration_seconds";
const PERSIST_FAILURE: &str = "indexer_persist_failure_total";

pub(crate) struct EventPersistorMetrics;

impl EventPersistorMetrics {
    /// Register once at startup.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            INSTRUCTIONS_INDEXED,
            "Instructions successfully parsed and indexed"
        );
        describe_histogram!(
            PERSIST_DURATION,
            "Duration of a single persist operation in seconds (label: kind)"
        );
        describe_counter!(
            PERSIST_FAILURE,
            "Failed persistence attempts per protocol and event kind"
        );
    }

    pub(crate) fn record_indexed(protocol: &Protocol, instruction: &str) {
        counter!(
            INSTRUCTIONS_INDEXED,
            "protocol" => protocol.as_str(),
            "instruction" => instruction.to_string(),
        )
        .increment(1);
    }

    /// `kind` labels the persist target: event kind ("swap", "liquidity",
    /// "claim_position_fee", "claim_reward") or pool-side operation
    /// ("pool_upsert", "pool_touch", "pool_current_state_applied",
    /// "pool_current_state_stale").
    pub(crate) fn record_persist_duration(protocol: &Protocol, kind: &'static str, seconds: f64) {
        histogram!(
            PERSIST_DURATION,
            "protocol" => protocol.as_str(),
            "kind" => kind,
        )
        .record(seconds);
    }

    pub(crate) fn record_persist_failure(protocol: &Protocol, event_kind: &'static str) {
        counter!(
            PERSIST_FAILURE,
            "protocol" => protocol.as_str().to_string(),
            "event_kind" => event_kind,
        )
        .increment(1);
    }
}
