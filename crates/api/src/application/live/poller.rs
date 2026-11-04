use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::LiveSource;
use super::watermark::LiveWatermark;

/// Rows read per tick, the repositories' page cap; a larger burst drains over the next ticks.
const POLL_BATCH_LIMIT: i64 = 200;

/// One source's poller.
pub(crate) struct LivePoller<S: LiveSource> {
    source: Arc<S>,
    sender: broadcast::Sender<Arc<S::Item>>,
    interval: Duration,
    /// The pollers' one read at a time, shared by all of them.
    reads: Arc<Semaphore>,
}

impl<S: LiveSource> LivePoller<S> {
    pub(crate) fn new(
        source: Arc<S>,
        sender: broadcast::Sender<Arc<S::Item>>,
        interval: Duration,
        reads: Arc<Semaphore>,
    ) -> Self {
        Self {
            source,
            sender,
            interval,
            reads,
        }
    }

    /// Polls until `shutdown` is cancelled; a failed tick is logged, never returned. The
    /// `Result` is for [`yog_bootstrap::Stop::settle`].
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        let mut ticker = tokio::time::interval(self.interval);
        // `None` = the watermark needs (re)anchoring at the table's tip.
        let mut watermark: Option<LiveWatermark> = None;
        info!(source = self.source.name(), interval = ?self.interval, "live poller started");

        loop {
            tokio::select! {
                // `biased`: a stop that arrives with a tick due must not start one more read.
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
                next = async {
                    let _read = self.reads.acquire().await;
                    poll_once(self.source.as_ref(), &self.sender, watermark).await
                } => next,
            };
        }
        Ok(())
    }
}

/// One tick: anchor if needed, read past `floor`, broadcast what is new, and return the
/// watermark — `None` when anchoring failed. Every failure is logged and skipped.
pub(super) async fn poll_once<S: LiveSource>(
    source: &S,
    sender: &broadcast::Sender<Arc<S::Item>>,
    watermark: Option<LiveWatermark>,
) -> Option<LiveWatermark> {
    let mut watermark = match watermark {
        Some(watermark) => watermark,
        None => match source.tip().await {
            Ok(tip) => LiveWatermark::anchored(tip.unwrap_or(0)),
            Err(e) => {
                warn!(source = source.name(), error = %e, "live poller: anchoring failed — skipping tick");
                return None;
            }
        },
    };

    let records = match source.after(watermark.floor(), POLL_BATCH_LIMIT).await {
        Ok(records) => records,
        Err(e) => {
            warn!(source = source.name(), error = %e, "live poller: read failed — skipping tick");
            return Some(watermark);
        }
    };

    let now = Instant::now();
    let fresh = watermark.fresh(records, S::id_of, now);
    watermark.settle(now);
    if fresh.is_empty() {
        return Some(watermark);
    }

    for item in source.prepare(fresh).await {
        // An error only means every receiver left since the count check.
        let _ = sender.send(Arc::new(item));
    }
    Some(watermark)
}

#[cfg(test)]
#[path = "poller_tests.rs"]
mod tests;
