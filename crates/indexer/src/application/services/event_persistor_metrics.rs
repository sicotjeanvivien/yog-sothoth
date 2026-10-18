//! Metrics emitted by the EventPersistor.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const INSTRUCTIONS_INDEXED: &str = "yog_indexer_instructions_indexed_total";
const PERSIST_DURATION: &str = "yog_indexer_persist_duration_seconds";
const PERSIST_FAILURE: &str = "yog_indexer_persist_failure_total";
const POOL_UPSERT_FAILURE: &str = "yog_indexer_pool_upsert_failure_total";
const INSERT_SKIPPED: &str = "yog_indexer_event_insert_skipped_total";
const PCS_SAME_SLOT: &str = "yog_indexer_pool_current_state_same_slot_total";

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
        describe_counter!(
            POOL_UPSERT_FAILURE,
            "Failed pool registry upserts; the event insert runs regardless"
        );
        describe_counter!(
            INSERT_SKIPPED,
            "Event inserts that hit ON CONFLICT DO NOTHING and wrote no row"
        );
        describe_counter!(
            PCS_SAME_SLOT,
            "Projection upserts that met state from the same slot under another \
             signature — the case the ordering key cannot rank"
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

    /// `kind` is the persist target: an event kind, or a pool-side operation (`pool_upsert`,
    /// `pool_touch`, `pool_mark_needs_refresh`, `pool_current_state_applied`, `pool_current_state_rejected`).
    pub(crate) fn record_persist_duration(protocol: &Protocol, kind: &'static str, seconds: f64) {
        histogram!(
            PERSIST_DURATION,
            "protocol" => protocol.as_str(),
            "kind" => kind,
        )
        .record(seconds);
    }

    /// An insert that conflicted and wrote nothing, counted beside `record_indexed`: rows written
    /// are `indexed − skipped`.
    pub(crate) fn record_insert_skipped(protocol: &Protocol, event_kind: &'static str) {
        counter!(
            INSERT_SKIPPED,
            "protocol" => protocol.as_str().to_string(),
            "event_kind" => event_kind,
        )
        .increment(1);
    }

    /// A projection upsert whose event shared its slot with the stored state under another
    /// signature, applied or rejected: without `transaction_index` that order is undecidable.
    /// ⚠️ Large by construction on a hot pool: read its ratio to `pool_current_state_applied`.
    pub(crate) fn record_pool_current_state_same_slot(protocol: &Protocol) {
        counter!(PCS_SAME_SLOT, "protocol" => protocol.as_str().to_string()).increment(1);
    }

    pub(crate) fn record_persist_failure(protocol: &Protocol, event_kind: &'static str) {
        counter!(
            PERSIST_FAILURE,
            "protocol" => protocol.as_str().to_string(),
            "event_kind" => event_kind,
        )
        .increment(1);
    }

    /// Not a `persist_failure`: the event insert still runs, only the `pools` write failed.
    pub(crate) fn record_pool_upsert_failure(protocol: &Protocol, event_kind: &'static str) {
        counter!(
            POOL_UPSERT_FAILURE,
            "protocol" => protocol.as_str().to_string(),
            "event_kind" => event_kind,
        )
        .increment(1);
    }
}
