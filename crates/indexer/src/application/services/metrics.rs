//! Metrics emitted by the IndexerService.

use metrics::{counter, describe_counter};
use yog_core::domain::Protocol;

const INSTRUCTIONS_SKIPPED: &str = "yog_indexer_instructions_skipped_total";
const INSTRUCTIONS_INDEXED: &str = "yog_indexer_instructions_indexed_total";
const TRANSACTIONS_NO_MATCH: &str = "yog_indexer_transactions_no_match_total";

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
    }

    /// Record an instruction that was identified by name but had no
    /// matching parser. The `instruction` label is the instruction
    /// variant name (e.g. "Swap2", "AddLiquidity").
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
}
