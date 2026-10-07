//! The ways a pricing cycle ends, each with its log line, its outcome label and,
//! for the endings that stop before pricing, a zeroed coverage gauge.
//!
//! ⚠️ The cycle *returns* a [`TickOutcome`], so leaving without saying so does
//! not compile; and one `match` keeps each ending's log and label together.

use std::time::Instant;

use tracing::{debug, info, warn};

use super::metrics::PriceWorkerMetrics;
use crate::error::SourceError;
use yog_core::RepositoryError;

/// How one pricing cycle ended.
pub(super) enum TickOutcome {
    /// The known-mint list could not be read, so the tick never started.
    ListFailed(RepositoryError),
    /// Nothing to price yet — an empty `token_metadata`, i.e. a cold start.
    NoKnownMints,
    /// Every known mint is waiting its turn (`UnpricedMints`).
    NothingDue,
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
    /// Say why the tick ended, and record its duration under its own label.
    /// If the stop came before the end, say also what became of what the tick
    /// had received.
    ///
    /// ⚠️ `no_prices` (the source valued nothing: an alarm) and `unchanged`
    /// (nothing moved: the normal case) must never share a label.
    pub(super) fn record(self, start: Instant, stopped: bool) {
        let written = match self {
            TickOutcome::Inserted { count } => count,
            _ => 0,
        };
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
            TickOutcome::NothingDue => {
                debug!("price worker: every known mint is waiting its turn — nothing asked");
                // Coverage 0 is the truth; `no_prices` would be the wrong alarm.
                no_coverage();
                "nothing_due"
            }
            TickOutcome::SourceFailed(e) => {
                warn!(error = %e, "price worker: source returned a hard error");
                // Unreachable with the Jupiter client; kept for the next source.
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

        if stopped {
            info!(
                outcome,
                written,
                "price worker: the stop came during this tick — outcome and written say \
                 what became of what it had received"
            );
        }

        PriceWorkerMetrics::record_tick(outcome, start.elapsed().as_secs_f64());
    }
}

/// The coverage numerator falls with its denominator, or `priced / known` reads
/// a stale ratio.
fn no_coverage() {
    PriceWorkerMetrics::set_priced_mints(0);
}
