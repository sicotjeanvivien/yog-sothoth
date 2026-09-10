//! Metrics emitted by the TransactionProcessorMetrics.
//!
//! ⚠️ **The `yog_indexer_fetch_*` families used to be here and are not any
//! more.** They moved to `infra::rpc::FetchMetrics` with the fetch itself,
//! which belongs to the one acquisition path that has to ask for a
//! transaction. Their exported names did not change; only the type that emits
//! them did. What that costs is written on `INDEX_TX_EXITED` below.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const TRANSACTIONS_NO_MATCH: &str = "yog_indexer_transactions_no_match_total";
const INDEX_TX_ENTERED: &str = "yog_indexer_index_transaction_entered_total";
const INDEX_TX_EXITED: &str = "yog_indexer_index_transaction_exited_total";
const UNKNOWN_EVENT: &str = "yog_indexer_unknown_event_total";
const EXTRACTION_FAILURE: &str = "yog_indexer_extraction_failure_total";

const INDEX_TX_DURATION: &str = "yog_indexer_index_transaction_duration_seconds";

pub(crate) struct TransactionProcessorMetrics;

impl TransactionProcessorMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TRANSACTIONS_NO_MATCH,
            "Transactions where no instruction was matched by any parser"
        );
        describe_counter!(INDEX_TX_ENTERED, "Calls to index_transaction (entry)");
        // ⚠️ `fetch_not_found` and `fetch_failure` are gone from this label set,
        // and that is the visible half of moving the fetch. Those two exits
        // happen before a transaction exists, so they are now counted where
        // they occur — `yog_indexer_fetch_not_found_total` and
        // `yog_indexer_fetch_failures_total`, same names, same labels, one
        // stage earlier.
        describe_counter!(
            INDEX_TX_EXITED,
            "Exits from index_transaction (label: outcome=ok|no_events|extract_failure|unknown_exit)"
        );

        // ⚠️ This histogram no longer includes the RPC round-trip. It measures
        // extract-and-persist, which is what both acquisition paths do
        // identically — so the two are comparable, which is the whole point of
        // keeping both. Fetch latency is `yog_indexer_fetch_duration_seconds`,
        // on the one path that has any.
        describe_histogram!(
            INDEX_TX_DURATION,
            "Total duration of index_transaction in seconds (label: outcome)"
        );
        describe_counter!(
            UNKNOWN_EVENT,
            "Anchor events extracted but not recognized — likely belong to rings not yet implemented"
        );
        describe_counter!(
            EXTRACTION_FAILURE,
            "Failed extraction attempts (decode / borsh / translation) per protocol and kind"
        );
    }

    pub(crate) fn record_no_match(protocol: &Protocol) {
        counter!(TRANSACTIONS_NO_MATCH, "protocol" => protocol.as_str()).increment(1);
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
            UNKNOWN_EVENT,
            "protocol" => protocol.as_str().to_string(),
            "discriminator" => discriminator_hex.to_string(),
        )
        .increment(1);
    }

    pub(crate) fn record_extraction_failure(protocol: &Protocol, kind: &'static str) {
        counter!(
            EXTRACTION_FAILURE,
            "protocol" => protocol.as_str().to_string(),
            "kind" => kind,
        )
        .increment(1);
    }
}
