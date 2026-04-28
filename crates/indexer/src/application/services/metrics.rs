//! Metrics emitted by the IndexerService.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

// Counters ────────────────────────────────────────────────────────────────────

const INSTRUCTIONS_SKIPPED: &str = "yog_indexer_instructions_skipped_total";
const INSTRUCTIONS_INDEXED: &str = "yog_indexer_instructions_indexed_total";
const TRANSACTIONS_NO_MATCH: &str = "yog_indexer_transactions_no_match_total";
const FETCH_FAILURES: &str = "yog_indexer_fetch_failures_total";
const FETCH_NOT_FOUND: &str = "yog_indexer_fetch_not_found_total";
const INDEX_TX_ENTERED: &str = "yog_indexer_index_transaction_entered_total";
const INDEX_TX_EXITED: &str = "yog_indexer_index_transaction_exited_total";

// Histograms ──────────────────────────────────────────────────────────────────

const FETCH_DURATION: &str = "yog_indexer_fetch_duration_seconds";
const PERSIST_DURATION: &str = "yog_indexer_persist_duration_seconds";
const INDEX_TX_DURATION: &str = "yog_indexer_index_transaction_duration_seconds";

pub(crate) struct IndexerServiceMetrics;

impl IndexerServiceMetrics {
    /// Register once at startup.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            INSTRUCTIONS_SKIPPED,
            "Instructions detected in a transaction but not matched by any parser"
        );
        describe_counter!(
            INSTRUCTIONS_INDEXED,
            "Instructions successfully parsed and indexed"
        );
        describe_counter!(
            TRANSACTIONS_NO_MATCH,
            "Transactions where no instruction was matched by any parser"
        );
        describe_counter!(
            FETCH_FAILURES,
            "Failures fetching a transaction from the RPC (label: reason)"
        );
        describe_counter!(
            FETCH_NOT_FOUND,
            "Transactions not found by the RPC after all retries"
        );
        describe_counter!(INDEX_TX_ENTERED, "Calls to index_transaction (entry)");
        describe_counter!(
            INDEX_TX_EXITED,
            "Exits from index_transaction (label: outcome=ok|fetch_not_found|fetch_failure|unknown_exit)"
        );

        describe_histogram!(
            FETCH_DURATION,
            "Duration of fetch_transaction in seconds (includes retries)"
        );
        describe_histogram!(
            PERSIST_DURATION,
            "Duration of a single persist operation in seconds (label: kind)"
        );
        describe_histogram!(
            INDEX_TX_DURATION,
            "Total duration of index_transaction in seconds (label: outcome)"
        );
        describe_counter!(
            "indexer_unknown_event_total",
            "Anchor events extracted but not recognized — likely belong to rings not yet implemented"
        );
        describe_counter!(
            "indexer_extraction_failure_total",
            "Failed extraction attempts (decode / borsh / translation) per protocol and kind"
        );
        describe_counter!(
            "indexer_persist_failure_total",
            "Failed persistence attempts per protocol and event kind"
        );
    }

    // Counters ────────────────────────────────────────────────────────────────

    pub(crate) fn record_skipped(protocol: &Protocol, instruction: &str) {
        counter!(
            INSTRUCTIONS_SKIPPED,
            "protocol" => protocol.as_str(),
            "instruction" => instruction.to_string(),
        )
        .increment(1);
    }

    pub(crate) fn record_indexed(protocol: &Protocol, instruction: &str) {
        counter!(
            INSTRUCTIONS_INDEXED,
            "protocol" => protocol.as_str(),
            "instruction" => instruction.to_string(),
        )
        .increment(1);
    }

    pub(crate) fn record_no_match(protocol: &Protocol) {
        counter!(TRANSACTIONS_NO_MATCH, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_fetch_failure(protocol: &Protocol, reason: &'static str) {
        counter!(FETCH_FAILURES, "protocol" => protocol.as_str(), "reason" => reason).increment(1);
    }

    pub(crate) fn record_fetch_not_found(protocol: &Protocol) {
        counter!(FETCH_NOT_FOUND, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_entered(protocol: &Protocol) {
        counter!(INDEX_TX_ENTERED, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_exited(protocol: &Protocol, outcome: &'static str) {
        counter!(
            INDEX_TX_EXITED,
            "protocol" => protocol.as_str(),
            "outcome" => outcome,
        )
        .increment(1);
    }

    // Histograms ──────────────────────────────────────────────────────────────

    pub(crate) fn record_fetch_duration(protocol: &Protocol, seconds: f64) {
        histogram!(FETCH_DURATION, "protocol" => protocol.as_str()).record(seconds);
    }

    /// `kind` labels the persist target: "pool_upsert", "swap", "metric",
    /// "liquidity_event".
    pub(crate) fn record_persist_duration(protocol: &Protocol, kind: &'static str, seconds: f64) {
        histogram!(
            PERSIST_DURATION,
            "protocol" => protocol.as_str(),
            "kind" => kind,
        )
        .record(seconds);
    }

    pub(crate) fn record_index_tx_duration(
        protocol: &Protocol,
        outcome: &'static str,
        seconds: f64,
    ) {
        histogram!(
            INDEX_TX_DURATION,
            "protocol" => protocol.as_str(),
            "outcome" => outcome,
        )
        .record(seconds);
    }

    /// Record an extracted but unrecognized Anchor event.
    ///
    /// `discriminator_hex` is the 16-character hex of the 8-byte
    /// discriminator. Bounded cardinality: the set of distinct
    /// unknown events is small (cercle 2/3 events not yet implemented).
    pub fn record_unknown_event(protocol: &Protocol, discriminator_hex: &str) {
        metrics::counter!(
            "indexer_unknown_event_total",
            "protocol" => protocol.as_str().to_string(),
            "discriminator" => discriminator_hex.to_string(),
        )
        .increment(1);
    }

    /// Record an extraction failure (anchor decode, borsh, translation).
    ///
    /// `kind` is one of: "anchor_decode", "borsh", "translation".
    pub fn record_extraction_failure(protocol: &Protocol, kind: &'static str) {
        metrics::counter!(
            "indexer_extraction_failure_total",
            "protocol" => protocol.as_str().to_string(),
            "kind" => kind,
        )
        .increment(1);
    }

    /// Record a persistence failure (post-extraction).
    pub fn record_persist_failure(protocol: &Protocol, event_kind: &'static str) {
        metrics::counter!(
            "indexer_persist_failure_total",
            "protocol" => protocol.as_str().to_string(),
            "event_kind" => event_kind,
        )
        .increment(1);
    }
}
