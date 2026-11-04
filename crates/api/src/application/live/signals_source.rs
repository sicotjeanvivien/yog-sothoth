use std::sync::Arc;

use tracing::warn;
use yog_core::RepositoryResult;
use yog_core::domain::{SignalFeed, SignalRecord};

use super::LiveSource;
use crate::application::{EnrichedSignal, SignalService};

/// How far back the signals are read live, in seconds.
/// ⚠️ A signal stamped further back when inserted is never streamed: the bound is what keeps
/// the read off the compressed chunks, and the paginated feed still lists it.
const SIGNALS_LOOKBACK_SECS: i64 = 86_400;

/// The `signals` table, each signal enriched with its pair.
pub(crate) struct SignalsSource {
    feed: Arc<dyn SignalFeed>,
    enricher: Arc<SignalService>,
}

impl SignalsSource {
    pub(crate) fn new(feed: Arc<dyn SignalFeed>, enricher: Arc<SignalService>) -> Self {
        Self { feed, enricher }
    }
}

impl LiveSource for SignalsSource {
    type Record = SignalRecord;
    type Item = EnrichedSignal;

    fn name(&self) -> &'static str {
        "signals"
    }

    async fn tip(&self) -> RepositoryResult<Option<i64>> {
        self.feed
            .live_tip(chrono::Duration::seconds(SIGNALS_LOOKBACK_SECS))
            .await
    }

    async fn after(&self, after_id: i64, limit: i64) -> RepositoryResult<Vec<SignalRecord>> {
        self.feed
            .live_after(
                after_id,
                chrono::Duration::seconds(SIGNALS_LOOKBACK_SECS),
                limit,
            )
            .await
    }

    fn id_of(record: &SignalRecord) -> i64 {
        record.id
    }

    /// On failure the alerts still go out, without their pairs: the paginated feed
    /// re-enriches them on the next load.
    async fn prepare(&self, records: Vec<SignalRecord>) -> Vec<EnrichedSignal> {
        match self.enricher.enrich_batch(records.clone()).await {
            Ok(enriched) => enriched,
            Err(e) => {
                warn!(error = %e, "live signals: token enrichment failed — emitting bare signals");
                records.into_iter().map(EnrichedSignal::bare).collect()
            }
        }
    }
}

#[cfg(test)]
#[path = "signals_source_tests.rs"]
mod tests;
