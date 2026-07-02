//! Background poller feeding the signal SSE stream.
//!
//! One shared task per process: it watches the `signals` feed's tip on a
//! fixed cadence and broadcasts every new row to the connected
//! `/api/signals/stream` clients (each connection is one `subscribe()`).
//! Chosen over LISTEN/NOTIFY: the table stays the only contract between
//! processes, nothing is ever lost across restarts (the watermark
//! re-anchors), and at the detectors' minutes-scale tempo a few seconds
//! of poll latency is invisible.
//!
//! The stream never replays history — a fresh watermark anchors at the
//! feed's tip, and the page loads its backlog through the paginated
//! `GET /api/signals` instead.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tracing::{info, warn};
use yog_core::domain::{SignalCursor, SignalFeedRepository, SignalRecord};

/// Capacity of the broadcast channel between the poller and the SSE
/// connections. A subscriber lagging further than this behind gets
/// `Lagged` and its stream is closed — the EventSource reconnects and
/// the page refetches, rather than silently missing alerts.
pub(crate) const STREAM_CHANNEL_CAPACITY: usize = 256;

/// Maximum rows fetched per tick. A larger burst is drained over the
/// following ticks — `newer_than` is watermark-driven, nothing is lost.
const POLL_BATCH_LIMIT: i64 = 256;

/// The shared feed poller. Holds the feed lens and the broadcast
/// sender; [`run`](Self::run) loops until the process dies (the api has
/// no graceful-shutdown path to hook into).
pub(crate) struct SignalStreamPoller {
    repo: Arc<dyn SignalFeedRepository>,
    sender: broadcast::Sender<SignalRecord>,
    interval: Duration,
}

impl SignalStreamPoller {
    pub(crate) fn new(
        repo: Arc<dyn SignalFeedRepository>,
        sender: broadcast::Sender<SignalRecord>,
        interval: Duration,
    ) -> Self {
        Self {
            repo,
            sender,
            interval,
        }
    }

    pub(crate) async fn run(self) {
        let mut ticker = tokio::time::interval(self.interval);
        // `None` = the watermark needs (re)anchoring at the feed's tip.
        let mut watermark: Option<SignalCursor> = None;
        info!(interval = ?self.interval, "signal stream poller started");

        loop {
            ticker.tick().await;

            // Nobody connected: skip the DB entirely, and drop the
            // watermark so the next active tick re-anchors at the tip
            // instead of replaying everything born during the idle gap.
            if self.sender.receiver_count() == 0 {
                watermark = None;
                continue;
            }

            watermark = poll_once(self.repo.as_ref(), &self.sender, watermark).await;
        }
    }
}

/// One poller tick: anchor the watermark if needed, fetch the delta,
/// broadcast it, and return the advanced watermark. Every failure is
/// skipped-and-logged — one missed tick, never a dead loop; `None`
/// means the anchoring itself failed and must be retried.
async fn poll_once(
    repo: &dyn SignalFeedRepository,
    sender: &broadcast::Sender<SignalRecord>,
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

    let mut next = anchor;
    for record in records {
        next = SignalCursor {
            triggered_at: record.signal.triggered_at,
            id: record.id,
        };
        // A send error only means every receiver disconnected since the
        // count check — benign; the watermark still advances (no
        // listeners, no delivery).
        let _ = sender.send(record);
    }
    Some(next)
}

#[cfg(test)]
#[path = "signal_stream_tests.rs"]
mod tests;
