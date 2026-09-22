//! The seven ways a pricing cycle ends, one function each.
//!
//! ⚠️ **The subject is the *ending*, not the logging.** The price worker logs
//! elsewhere too — the size of the batch it is about to ask for, the mints it
//! refused as unstorable — and none of that belongs here. What does is the
//! triple every exit owes: say why, stamp a duration under this ending's own
//! outcome label, and for the two that stop before anything was priced, zero
//! the coverage gauge so its numerator never outlives the denominator it was
//! measured against.
//!
//! Written inline, that triple was the same three lines repeated at seven
//! `return`s in the middle of the algorithm — and it showed: `no_prices` was
//! declared in the label set from the start and emitted at none of them, and
//! `set_priced_mints(0)` was missing from two of the three early exits. One
//! call per ending is what makes half-applying it impossible.
//!
//! What stays in the worker is what is **not** an ending: the counters several
//! exits share, and the coverage gauge, whose *position* between the two
//! filters is a decision rather than a consequence.

use std::time::Instant;

use tracing::{debug, warn};

use super::price_metrics::PriceWorkerMetrics;
use crate::error::SourceError;
use yog_core::RepositoryError;

/// The known-mint list could not be read, so the tick never started.
pub(super) fn list_failed(start: Instant, error: &RepositoryError) {
    warn!(error = %error, "price worker: list_known_mints failed");
    record(start, "list_failed");
}

/// Nothing to price yet — an empty `token_metadata`, i.e. a cold start.
pub(super) fn no_known_mints(start: Instant) {
    debug!("price worker: no known mints yet — sleeping");
    no_coverage();
    record(start, "no_work");
}

/// The source returned a hard error rather than a partial answer.
///
/// Unreachable with the current Jupiter client, which absorbs per-chunk
/// failures and returns `Ok(partial)` — which is exactly why the gauge reset
/// would rot silently here in the next `PriceSource`.
pub(super) fn source_failed(start: Instant, error: &SourceError) {
    warn!(error = %error, "price worker: source returned a hard error");
    no_coverage();
    record(start, "source_hard_error");
}

/// Prices came back, but none the price column can hold.
pub(super) fn no_storable_price(start: Instant) {
    debug!("price worker: no prices to insert");
    record(start, "no_prices");
}

/// Every price repeats the last one kept for its mint.
///
/// ⚠️ **The normal case, and never `no_prices`.** That label says the source
/// valued nothing, which is an anomaly worth alerting on; this one says
/// everything it valued was already on record, which after the redundancy
/// filter is what most ticks do. Sharing a label would leave that alert lit
/// for ever.
pub(super) fn all_unchanged(start: Instant, suppressed: usize) {
    debug!(
        count = suppressed,
        "price worker: every price repeats the last one kept"
    );
    record(start, "unchanged");
}

/// The batch was refused by the database.
pub(super) fn insert_failed(start: Instant, error: &RepositoryError) {
    warn!(error = %error, "price worker: insert_batch failed");
    record(start, "insert_failed");
}

/// Rows were written.
pub(super) fn inserted(start: Instant, count: usize) {
    PriceWorkerMetrics::record_inserted(count);
    debug!(count, "price worker: prices inserted");
    record(start, "ok");
}

/// Both gauges move together or the ratio the README tells you to alert on
/// (`priced / known`) divides a stale numerator by 0 and reads +Inf on a cold
/// start.
fn no_coverage() {
    PriceWorkerMetrics::set_priced_mints(0);
}

/// One duration, taken the same way for every ending — including the ones that
/// return after a single failed query.
fn record(start: Instant, outcome: &'static str) {
    PriceWorkerMetrics::record_tick(outcome, start.elapsed().as_secs_f64());
}
