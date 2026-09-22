//! The seven ways a **pricing** cycle ends, and everything the outside learns
//! from each.
//!
//! Named after its type rather than after the worker, unlike its neighbours in
//! this folder: `TickOutcome` is what a reader follows here from `price.rs`,
//! and the day another worker needs one, its variants will be different enough
//! to want their own type and their own name.
//!
//! ⚠️ **The type exists so that leaving without saying so cannot compile.**
//! Every exit owes the same triple: a reason in the log, a duration stamped
//! under this ending's own outcome label, and — for the two that stop before
//! anything was priced — a zeroed coverage gauge, so its numerator never
//! outlives the denominator it was measured against. Written inline at seven
//! `return`s, that triple is a convention, and this file's own history is the
//! argument against conventions: `no_prices` was declared in the label set from
//! the start and emitted at none of them, and `set_priced_mints(0)` was missing
//! from two of the three early exits.
//!
//! Because the cycle *returns* a [`TickOutcome`], a bare `return;` no longer
//! type-checks. An eighth ending is a new variant, and the compiler asks for
//! its arm rather than a reviewer noticing its absence.
//!
//! **One `match`, on purpose.** An earlier draft had two — one for the log, one
//! for the label — which put the two halves of an ending in different places
//! and let them drift: giving `AllUnchanged` the label of `NoStorablePrice`
//! would have compiled. Here each arm logs *and* evaluates to its own label, so
//! there is one place per ending and nothing to keep in step.
//!
//! What stays in the worker is what is **not** an ending: the counters several
//! exits share, and the coverage gauge, whose *position* between the two
//! filters is a decision rather than a consequence.

use std::time::Instant;

use tracing::{debug, warn};

use super::price_metrics::PriceWorkerMetrics;
use crate::error::SourceError;
use yog_core::RepositoryError;

/// How one pricing cycle ended.
///
/// Every variant is terminal: the cycle yields one and does nothing more.
pub(super) enum TickOutcome {
    /// The known-mint list could not be read, so the tick never started.
    ListFailed(RepositoryError),
    /// Nothing to price yet — an empty `token_metadata`, i.e. a cold start.
    NoKnownMints,
    /// The source returned a hard error rather than a partial answer.
    SourceFailed(SourceError),
    /// Prices came back, but none the price column can hold.
    NoStorablePrice,
    /// Every price repeats the last one kept for its mint.
    AllUnchanged { suppressed: usize },
    /// The batch was refused by the database.
    InsertFailed(RepositoryError),
    /// Rows were written.
    Inserted { count: usize },
}

impl TickOutcome {
    /// Say why the tick ended, and record it under its own outcome label.
    ///
    /// Takes the `Instant` rather than a duration so that every ending — down
    /// to the one that returns after a single failed query — is timed the same
    /// way, by the same line.
    ///
    /// ⚠️ `no_prices` and `unchanged` are **not** the same event and must never
    /// share a label. The first says the source valued nothing, an anomaly
    /// worth alerting on; the second says everything it valued was already on
    /// record, which after the redundancy filter is what most ticks do.
    /// Sharing a label would leave that alert lit for ever.
    pub(super) fn record(self, start: Instant) {
        let outcome = match self {
            TickOutcome::ListFailed(e) => {
                warn!(error = %e, "price worker: list_known_mints failed");
                "list_failed"
            }
            TickOutcome::NoKnownMints => {
                debug!("price worker: no known mints yet — sleeping");
                no_coverage();
                "no_work"
            }
            TickOutcome::SourceFailed(e) => {
                warn!(error = %e, "price worker: source returned a hard error");
                // Unreachable with the current Jupiter client, which absorbs
                // per-chunk failures and returns `Ok(partial)` — which is
                // exactly why the gauge reset would rot silently here in the
                // next `PriceSource`.
                no_coverage();
                "source_hard_error"
            }
            TickOutcome::NoStorablePrice => {
                debug!("price worker: no prices to insert");
                "no_prices"
            }
            TickOutcome::AllUnchanged { suppressed } => {
                debug!(
                    count = suppressed,
                    "price worker: every price repeats the last one kept"
                );
                "unchanged"
            }
            TickOutcome::InsertFailed(e) => {
                warn!(error = %e, "price worker: insert_batch failed");
                "insert_failed"
            }
            TickOutcome::Inserted { count } => {
                PriceWorkerMetrics::record_inserted(count);
                debug!(count, "price worker: prices inserted");
                "ok"
            }
        };

        PriceWorkerMetrics::record_tick(outcome, start.elapsed().as_secs_f64());
    }
}

/// Both gauges move together or the ratio the README tells you to alert on
/// (`priced / known`) divides a stale numerator by 0 and reads +Inf on a cold
/// start.
fn no_coverage() {
    PriceWorkerMetrics::set_priced_mints(0);
}
