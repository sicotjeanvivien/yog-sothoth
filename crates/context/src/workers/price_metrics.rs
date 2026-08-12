//! Metrics emitted by the price worker.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const TICK_TOTAL: &str = "yog_context_price_tick_total";
const TICK_DURATION: &str = "yog_context_price_tick_duration_seconds";
const KNOWN_MINTS: &str = "yog_context_price_known_mints";
const PRICED_MINTS: &str = "yog_context_price_priced_mints";
const INSERTED_TOTAL: &str = "yog_context_price_inserted_total";
const REJECTED_TOTAL: &str = "yog_context_price_rejected_total";

pub(crate) struct PriceWorkerMetrics;

impl PriceWorkerMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Price worker ticks completed (label: outcome=ok|no_work|list_failed|source_hard_error|no_prices|insert_failed). \
             `no_prices` covers both a source that priced nothing and a tick whose every price was rejected as unstorable — \
             yog_context_price_rejected_total tells the two apart"
        );
        describe_histogram!(
            TICK_DURATION,
            "Total duration of a price worker tick in seconds (label: outcome)"
        );
        describe_gauge!(
            KNOWN_MINTS,
            "Number of known mints submitted to the price source at the last tick"
        );
        describe_gauge!(
            PRICED_MINTS,
            "Number of those mints that yielded a price we KEPT at the last tick. \
             priced/known is the price coverage — the input every USD valuation \
             downstream depends on, and the thing that silently degrades. \
             Counted after the unstorable-price filter: a price the source \
             returned but we refused is, downstream, as absent as one it never \
             returned"
        );
        describe_counter!(
            INSERTED_TOTAL,
            "Token prices successfully inserted (cumulative count of rows)"
        );
        describe_counter!(
            REJECTED_TOTAL,
            "Prices dropped before insert because the NUMERIC(38, 18) column \
             cannot hold them — below 5e-19 (would store as zero) or at/above \
             1e20 (would overflow the type). Expected to stay at 0; a rising \
             count means either a very-high-supply mint entered the known set or \
             the source returned an absurd value, and that the affected USD \
             figures are absent rather than wrong"
        );

        // Materialise it at zero. `describe_counter!` only registers the help
        // text: the Prometheus exporter emits nothing for a counter that has
        // never been incremented, so a metric expected to sit at 0 for ever
        // would be *absent* for ever — unalertable, and indistinguishable from
        // a build where the rejection path was dropped. Publishing the zero is
        // what makes "flat at 0" an observation instead of a hope.
        counter!(REJECTED_TOTAL).absolute(0);
    }

    pub(crate) fn record_tick(outcome: &'static str, seconds: f64) {
        counter!(TICK_TOTAL, "outcome" => outcome).increment(1);
        histogram!(TICK_DURATION, "outcome" => outcome).record(seconds);
    }

    pub(crate) fn set_known_mints(count: usize) {
        gauge!(KNOWN_MINTS).set(count as f64);
    }

    /// Set alongside [`Self::set_known_mints`] on every tick that reached the
    /// source, including the zero case — a gauge left at its previous value
    /// would report yesterday's coverage as today's.
    pub(crate) fn set_priced_mints(count: usize) {
        gauge!(PRICED_MINTS).set(count as f64);
    }

    pub(crate) fn record_inserted(count: usize) {
        counter!(INSERTED_TOTAL).increment(count as u64);
    }

    /// Count prices refused by [`TokenPrice::is_storable`][is_storable] before
    /// the batch insert — at either end of the column. Skip-and-log, and the
    /// skip is countable.
    ///
    /// [is_storable]: yog_core::domain::TokenPrice::is_storable
    pub(crate) fn record_rejected(count: usize) {
        counter!(REJECTED_TOTAL).increment(count as u64);
    }
}
