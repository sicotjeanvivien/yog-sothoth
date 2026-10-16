//! The background poller feeding the signal SSE stream: one task per process enriches each new
//! signal once and broadcasts it to every `/api/signals/stream` client. Polling, not
//! LISTEN/NOTIFY: the table stays the only contract, and the watermark re-anchors after a
//! restart. ⚠️ The stream never replays history: the page loads it through `GET /api/signals`.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use yog_core::domain::{SignalCursor, SignalFeed};

use crate::application::{EnrichedSignal, SignalService};

/// Capacity of the broadcast channel. A subscriber lagging further gets `Lagged` and its
/// stream closes: the page reconnects and refetches rather than silently missing alerts.
pub(crate) const STREAM_CHANNEL_CAPACITY: usize = 256;

/// Rows fetched per tick; a larger burst drains over the next ticks, nothing lost.
const POLL_BATCH_LIMIT: i64 = 256;

/// The shared feed poller.
pub(crate) struct SignalStreamPoller {
    repo: Arc<dyn SignalFeed>,
    enricher: Arc<SignalService>,
    sender: broadcast::Sender<Arc<EnrichedSignal>>,
    interval: Duration,
}

impl SignalStreamPoller {
    pub(crate) fn new(
        repo: Arc<dyn SignalFeed>,
        enricher: Arc<SignalService>,
        sender: broadcast::Sender<Arc<EnrichedSignal>>,
        interval: Duration,
    ) -> Self {
        Self {
            repo,
            enricher,
            sender,
            interval,
        }
    }

    /// Polls until `shutdown` is cancelled; a failed tick is logged, never returned. The
    /// `Result` is for [`yog_bootstrap::Stop::settle`].
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        let mut ticker = tokio::time::interval(self.interval);
        // `None` = the watermark needs (re)anchoring at the feed's tip.
        let mut watermark: Option<SignalCursor> = None;
        info!(interval = ?self.interval, "signal stream poller started");

        loop {
            tokio::select! {
                // `biased`: a stop that arrives with a tick due must not start
                // one more read of the feed.
                biased;

                () = shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }

            // Nobody connected: no read, and the watermark dropped so the next tick
            // re-anchors at the tip instead of replaying the idle gap.
            if self.sender.receiver_count() == 0 {
                watermark = None;
                continue;
            }

            // ⚠️ The tick answers the stop too: its queries could outlast `SHUTDOWN_GRACE`.
            watermark = tokio::select! {
                biased;

                () = shutdown.cancelled() => break,
                next = poll_once(self.repo.as_ref(), &self.enricher, &self.sender, watermark) => next,
            };
        }
        Ok(())
    }
}

/// One tick: anchor if needed, fetch, enrich in one batch, broadcast, and return the advanced
/// watermark — `None` when anchoring failed. Every failure is logged and skipped.
async fn poll_once(
    repo: &dyn SignalFeed,
    enricher: &SignalService,
    sender: &broadcast::Sender<Arc<EnrichedSignal>>,
    watermark: Option<SignalCursor>,
) -> Option<SignalCursor> {
    let anchor = match watermark {
        Some(w) => w,
        None => match repo.latest_cursor().await {
            Ok(Some(tip)) => tip,
            // Empty feed: anchor at the origin — everything to come is new.
            Ok(None) => SignalCursor {
                triggered_at: DateTime::<Utc>::UNIX_EPOCH,
                id: 0,
            },
            Err(e) => {
                warn!(error = %e, "signal stream: watermark anchoring failed — skipping tick");
                return None;
            }
        },
    };

    let records = match repo.newer_than(&anchor, POLL_BATCH_LIMIT).await {
        Ok(records) => records,
        Err(e) => {
            warn!(error = %e, "signal stream: feed read failed — skipping tick");
            return Some(anchor);
        }
    };

    // The watermark follows the feed, whatever the enrichment does.
    let next = records.last().map_or(anchor, |record| SignalCursor {
        triggered_at: record.signal.triggered_at,
        id: record.id,
    });

    // On failure the alerts still go out, without their pairs: a failed lookup leaves the
    // whole tick bare, and the paginated feed re-enriches them on the next load.
    let enriched = match enricher.enrich_batch(records.clone()).await {
        Ok(enriched) => enriched,
        Err(e) => {
            warn!(error = %e, "signal stream: token enrichment failed — emitting bare signals");
            records.into_iter().map(EnrichedSignal::bare).collect()
        }
    };

    for signal in enriched {
        // An error only means every receiver left since the count check.
        let _ = sender.send(Arc::new(signal));
    }
    Some(next)
}

#[cfg(test)]
#[path = "signal_stream_tests.rs"]
mod tests;
