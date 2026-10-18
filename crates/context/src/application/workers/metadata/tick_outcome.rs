//! The ways a metadata tick ends, each with its log line and its outcome label.

use tokio::time::Instant;
use tracing::{debug, warn};

use super::metrics::MetadataWorkerMetrics;
use super::scope::Scope;
use crate::error::SourceError;
use yog_core::RepositoryError;

/// How one metadata tick ended.
pub(super) enum TickOutcome {
    /// The missing mints could not be listed.
    ListFailed(RepositoryError),
    /// No mint of the scope lacks metadata.
    NoWork,
    /// The source returned a hard error rather than a partial answer.
    SourceFailed(SourceError),
    /// What the source returned was upserted, row by row.
    Enriched,
}

impl TickOutcome {
    /// Say why the tick ended, and record its duration under its label and scope.
    pub(super) fn record(self, scope: Scope, start: Instant) {
        let outcome = match self {
            TickOutcome::ListFailed(e) => {
                warn!(
                    error = %e,
                    scope = scope.as_label(),
                    "metadata worker: listing missing mints failed",
                );
                "list_failed"
            }
            TickOutcome::NoWork => {
                debug!(
                    scope = scope.as_label(),
                    "metadata worker: no missing mints — sleeping"
                );
                "no_work"
            }
            TickOutcome::SourceFailed(e) => {
                warn!(
                    error = %e,
                    scope = scope.as_label(),
                    "metadata worker: source returned a hard error",
                );
                "source_hard_error"
            }
            TickOutcome::Enriched => "ok",
        };

        MetadataWorkerMetrics::record_tick(
            outcome,
            scope.as_label(),
            start.elapsed().as_secs_f64(),
        );
    }
}
