//! Metrics emitted by the EventPersistor.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const INSTRUCTIONS_INDEXED: &str = "yog_indexer_instructions_indexed_total";
const PERSIST_DURATION: &str = "yog_indexer_persist_duration_seconds";
const PERSIST_FAILURE: &str = "yog_indexer_persist_failure_total";
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

    /// `kind` labels the persist target: event kind ("swap", "liquidity",
    /// "claim_position_fee", "claim_reward") or pool-side operation
    /// ("pool_upsert", "pool_touch", "pool_current_state_applied",
    /// "pool_current_state_rejected").
    pub(crate) fn record_persist_duration(protocol: &Protocol, kind: &'static str, seconds: f64) {
        histogram!(
            PERSIST_DURATION,
            "protocol" => protocol.as_str(),
            "kind" => kind,
        )
        .record(seconds);
    }

    /// An insert that conflicted and wrote nothing.
    ///
    /// Counted **in addition to** `record_indexed`, never instead of it: that
    /// counter keeps meaning "events processed", and rows actually written are
    /// `indexed − skipped`. Redefining an existing counter in place would make
    /// every historical comparison silently wrong.
    ///
    /// A non-zero rate on a live stream means the unique key is collapsing
    /// distinct events — the failure mode that made this counter necessary.
    /// On a replay it is the expected outcome.
    pub(crate) fn record_insert_skipped(protocol: &Protocol, event_kind: &'static str) {
        counter!(
            INSERT_SKIPPED,
            "protocol" => protocol.as_str().to_string(),
            "event_kind" => event_kind,
        )
        .increment(1);
    }

    /// A projection upsert whose incoming event shared its slot with the
    /// state already stored, under a different signature.
    ///
    /// `transaction_index` is empty on the `getTransaction` ingestion path, so
    /// `(slot, _, event_index)` cannot rank two transactions of one block. The
    /// counter fires on both outcomes — applied and rejected — because an
    /// ambiguity that wrongly accepts costs as much as one that wrongly
    /// rejects, and counting only rejections would understate it.
    ///
    /// **Do not read it as an alarm threshold.** On a hot pool it will be
    /// large by construction: the audit measured up to 46 swaps of one pool
    /// within a second, so ~18 per 400 ms slot — same-slot collisions there are
    /// structural, not residual. An earlier version of this comment said
    /// "expected to stay near zero", which would have made a normal reading
    /// look like an incident.
    ///
    /// The signal is its **ratio to `pool_current_state_applied`**, per pool.
    /// Stated in advance so the conclusion is not fitted to the number: above
    /// roughly 10 % of applied upserts on a pool that matters, the ordering is
    /// deciding too much by a rule that cannot decide, and reading
    /// `transaction_index` from `getBlock` stops being optional.
    ///
    /// It is also a **lower bound**: under concurrent writers the repository
    /// can miss an ambiguity — see `PgPoolCurrentStateRepository::upsert`.
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
}
