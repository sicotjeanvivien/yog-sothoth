//! Metrics emitted by the price worker.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const TICK_TOTAL: &str = "yog_context_price_tick_total";
const TICK_DURATION: &str = "yog_context_price_tick_duration_seconds";
const CANDIDATE_MINTS: &str = "yog_context_price_candidate_mints";
const CANDIDATES_BY_CLASS: &str = "yog_context_price_candidates_by_class";
const REQUESTED_MINTS: &str = "yog_context_price_requested_mints";
const PRICED_MINTS: &str = "yog_context_price_priced_mints";
const INSERTED_TOTAL: &str = "yog_context_price_inserted_total";
const REJECTED_TOTAL: &str = "yog_context_price_rejected_total";
const UNCHANGED_TOTAL: &str = "yog_context_price_unchanged_total";

pub(crate) struct PriceWorkerMetrics;

impl PriceWorkerMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Price worker ticks completed (label: outcome=ok|no_work|nothing_due|list_failed|source_hard_error|no_prices|unchanged|insert_failed). \
             `nothing_due` is a tick that asked nothing because every candidate mint is one the source last answered \
             without a price and is waiting its turn. \
             `no_prices` covers both a source that priced nothing and a tick whose every price was rejected as unstorable — \
             yog_context_price_rejected_total tells the two apart. `unchanged` is the opposite and is the NORMAL case: \
             every price came back identical to the last one kept, so the tick wrote nothing on purpose"
        );
        describe_histogram!(
            TICK_DURATION,
            "Total duration of a price worker tick in seconds (label: outcome)"
        );
        describe_gauge!(
            CANDIDATE_MINTS,
            "Number of candidate mints at the last tick: known mints (token_metadata) that one \
             of their pools moved, or that the source priced, within PRICE_CANDIDATE_WINDOW. \
             A mint quiet on both counts is not asked"
        );
        describe_gauge!(
            CANDIDATES_BY_CLASS,
            "Candidate mints at the last tick by class (label: class=regular|low_value|\
             never_priced), which sets how often each is asked — see PriceCandidate"
        );
        describe_gauge!(
            REQUESTED_MINTS,
            "Number of candidate mints actually asked of the price source at the last tick. \
             The rest are mints out of their turn (see PriceCandidate), or the source last \
             answered without a price and waiting (at most \
             unpriced_wait_cap_secs, stated at startup, plus one cycle: see \
             UnpricedMints). \
             requested/candidate is the share of the candidates due this tick"
        );
        describe_gauge!(
            PRICED_MINTS,
            "Number of candidate mints that yielded a price we KEPT at the last tick. \
             priced/requested is the price coverage — the input every USD valuation \
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

        describe_counter!(
            UNCHANGED_TOTAL,
            "Prices fetched but not written because they repeat the last observation kept for \
             that mint and it is still recent. Expected to carry four rows in five: that is the \
             point, not a fault. rate(unchanged)/(rate(unchanged)+rate(inserted)) is the \
             suppression rate of the series — rates, not the bare counters, which average over \
             the process lifetime and hide the collapse. A collapse towards 0 means the market \
             moved, or a tick outlasted half the floor, or the comparison \
             stopped rounding to the price column's scale — see KeptPrices::worth_keeping"
        );

        // ⚠️ Published at zero: a counter never incremented is not exported, and "flat at 0" would
        // read as absent.
        counter!(REJECTED_TOTAL).absolute(0);
        counter!(UNCHANGED_TOTAL).absolute(0);
        counter!(INSERTED_TOTAL).absolute(0);
    }

    pub(crate) fn record_tick(outcome: &'static str, seconds: f64) {
        counter!(TICK_TOTAL, "outcome" => outcome).increment(1);
        histogram!(TICK_DURATION, "outcome" => outcome).record(seconds);
    }

    pub(crate) fn set_candidate_mints(count: usize) {
        gauge!(CANDIDATE_MINTS).set(count as f64);
    }

    pub(crate) fn set_candidates_by_class(class: &'static str, count: usize) {
        gauge!(CANDIDATES_BY_CLASS, "class" => class).set(count as f64);
    }

    /// Set with [`Self::set_candidate_mints`], on every tick that read the
    /// candidates, zero included.
    pub(crate) fn set_requested_mints(count: usize) {
        gauge!(REQUESTED_MINTS).set(count as f64);
    }

    /// Set on every tick, zero included: a stale value would report yesterday's
    /// coverage as today's.
    pub(crate) fn set_priced_mints(count: usize) {
        gauge!(PRICED_MINTS).set(count as f64);
    }

    pub(crate) fn record_inserted(count: usize) {
        counter!(INSERTED_TOTAL).increment(count as u64);
    }

    /// Count prices refused by [`TokenPrice::is_storable`][is_storable].
    ///
    /// [is_storable]: yog_core::domain::TokenPrice::is_storable
    pub(crate) fn record_rejected(count: usize) {
        counter!(REJECTED_TOTAL).increment(count as u64);
    }

    /// Count prices the redundancy rule ([`KeptPrices`][kept]) suppressed.
    ///
    /// [kept]: yog_core::domain::KeptPrices
    pub(crate) fn record_unchanged(count: usize) {
        counter!(UNCHANGED_TOTAL).increment(count as u64);
    }
}
