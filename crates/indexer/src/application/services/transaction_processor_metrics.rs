//! Metrics emitted by the TransactionProcessorMetrics.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const TRANSACTIONS_NO_MATCH: &str = "yog_indexer_transactions_no_match_total";
const FETCH_FAILURES: &str = "yog_indexer_fetch_failures_total";
const FETCH_NOT_FOUND: &str = "yog_indexer_fetch_not_found_total";
const INDEX_TX_ENTERED: &str = "yog_indexer_index_transaction_entered_total";
const INDEX_TX_EXITED: &str = "yog_indexer_index_transaction_exited_total";

const FETCH_DURATION: &str = "yog_indexer_fetch_duration_seconds";
const INDEX_TX_DURATION: &str = "yog_indexer_index_transaction_duration_seconds";

pub(crate) struct TransactionProcessorMetrics;

impl TransactionProcessorMetrics {
    pub(crate) fn register_descriptions() {
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
            INDEX_TX_DURATION,
            "Total duration of index_transaction in seconds (label: outcome)"
        );
        describe_counter!(
            "yog_indexer_unknown_event_total",
            "Anchor events extracted but not recognized — likely belong to rings not yet implemented"
        );
        describe_counter!(
            "yog_indexer_extraction_failure_total",
            "Failed extraction attempts (decode / borsh / translation) per protocol and kind"
        );
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

    pub(crate) fn record_fetch_duration(protocol: &Protocol, seconds: f64) {
        histogram!(FETCH_DURATION, "protocol" => protocol.as_str()).record(seconds);
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

    pub(crate) fn record_unknown_event(protocol: &Protocol, discriminator_hex: &str) {
        counter!(
            "indexer_unknown_event_total",
            "protocol" => protocol.as_str().to_string(),
            "discriminator" => discriminator_hex.to_string(),
        )
        .increment(1);
    }

    pub(crate) fn record_extraction_failure(protocol: &Protocol, kind: &'static str) {
        counter!(
            "indexer_extraction_failure_total",
            "protocol" => protocol.as_str().to_string(),
            "kind" => kind,
        )
        .increment(1);
    }
}
