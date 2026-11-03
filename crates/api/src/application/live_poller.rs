//! The pollers feeding `/api/live`: one task per source walks its table by id, reads only while
//! someone listens, and broadcasts each new item once to every connection. Polling, not
//! LISTEN/NOTIFY: the table stays the only contract. ⚠️ Nothing is replayed: a page loads the
//! history through the source's collection.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use yog_core::RepositoryResult;
use yog_core::domain::{SignalFeed, SignalRecord};

use crate::application::{EnrichedSignal, SignalService};

/// Capacity of each source's broadcast channel. A connection lagging further gets `Lagged` and
/// closes: the page reconnects and refetches rather than silently missing items.
pub(crate) const LIVE_CHANNEL_CAPACITY: usize = 256;

/// Rows read per tick, the repositories' page cap; a larger burst drains over the next ticks.
const POLL_BATCH_LIMIT: i64 = 200;

/// How long a hole under an id already read is waited for before it is passed over.
/// ⚠️ Ids are taken at insert, not at commit: a slower insert lands below an id already read.
const GAP_SETTLE: Duration = Duration::from_secs(10);

/// A table `/api/live` streams: read by id, then made into the items it broadcasts.
pub(crate) trait LiveSource: Send + Sync + 'static {
    type Record: Send;
    type Item: Send + Sync + 'static;

    /// Its name in the logs.
    const NAME: &'static str;

    /// The highest id there is, `None` on an empty table.
    fn tip(&self) -> impl Future<Output = RepositoryResult<Option<i64>>> + Send;

    /// Records with an id above `after_id`, by id, at most `limit`.
    fn after(
        &self,
        after_id: i64,
        limit: i64,
    ) -> impl Future<Output = RepositoryResult<Vec<Self::Record>>> + Send;

    fn id_of(record: &Self::Record) -> i64;

    /// The items to broadcast, one per record and in its order; it fails nothing.
    fn prepare(&self, records: Vec<Self::Record>) -> impl Future<Output = Vec<Self::Item>> + Send;
}

/// Where a poller stands in its table: every id up to `floor` is settled, and those above it
/// already delivered are kept, with when they were read, so that a hole filled later is
/// delivered without repeating them.
#[derive(Debug)]
pub(crate) struct LiveWatermark {
    floor: i64,
    delivered: BTreeMap<i64, Instant>,
}

impl LiveWatermark {
    pub(crate) fn anchored(tip: i64) -> Self {
        Self {
            floor: tip,
            delivered: BTreeMap::new(),
        }
    }

    pub(crate) fn floor(&self) -> i64 {
        self.floor
    }

    /// The records not delivered yet, now counted as delivered, read at `now`.
    pub(crate) fn fresh<R>(
        &mut self,
        records: Vec<R>,
        id_of: impl Fn(&R) -> i64,
        now: Instant,
    ) -> Vec<R> {
        records
            .into_iter()
            .filter(|record| {
                let id = id_of(record);
                id > self.floor && self.delivered.insert(id, now).is_none()
            })
            .collect()
    }

    /// Moves `floor` over the ids delivered without a hole, and over a hole under an id read
    /// `GAP_SETTLE` ago: each hole is dated by the id above it, not by when the floor reached it.
    pub(crate) fn settle(&mut self, now: Instant) {
        loop {
            while self
                .delivered
                .first_key_value()
                .is_some_and(|(&id, _)| id == self.floor + 1)
            {
                self.delivered.pop_first();
                self.floor += 1;
            }
            let Some((&next, &read_at)) = self.delivered.first_key_value() else {
                return;
            };
            if now.duration_since(read_at) < GAP_SETTLE {
                return;
            }
            self.floor = next - 1;
        }
    }
}

/// One source's poller.
pub(crate) struct LivePoller<S: LiveSource> {
    source: Arc<S>,
    sender: broadcast::Sender<Arc<S::Item>>,
    interval: Duration,
}

impl<S: LiveSource> LivePoller<S> {
    pub(crate) fn new(
        source: Arc<S>,
        sender: broadcast::Sender<Arc<S::Item>>,
        interval: Duration,
    ) -> Self {
        Self {
            source,
            sender,
            interval,
        }
    }

    /// Polls until `shutdown` is cancelled; a failed tick is logged, never returned. The
    /// `Result` is for [`yog_bootstrap::Stop::settle`].
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        let mut ticker = tokio::time::interval(self.interval);
        // `None` = the watermark needs (re)anchoring at the table's tip.
        let mut watermark: Option<LiveWatermark> = None;
        info!(source = S::NAME, interval = ?self.interval, "live poller started");

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
                next = poll_once(self.source.as_ref(), &self.sender, watermark) => next,
            };
        }
        Ok(())
    }
}

/// One tick: anchor if needed, read past `floor`, broadcast what is new, and return the
/// watermark — `None` when anchoring failed. Every failure is logged and skipped.
pub(crate) async fn poll_once<S: LiveSource>(
    source: &S,
    sender: &broadcast::Sender<Arc<S::Item>>,
    watermark: Option<LiveWatermark>,
) -> Option<LiveWatermark> {
    let mut watermark = match watermark {
        Some(watermark) => watermark,
        None => match source.tip().await {
            Ok(tip) => LiveWatermark::anchored(tip.unwrap_or(0)),
            Err(e) => {
                warn!(source = S::NAME, error = %e, "live poller: anchoring failed — skipping tick");
                return None;
            }
        },
    };

    let records = match source.after(watermark.floor(), POLL_BATCH_LIMIT).await {
        Ok(records) => records,
        Err(e) => {
            warn!(source = S::NAME, error = %e, "live poller: read failed — skipping tick");
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

    const NAME: &'static str = "signals";

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
#[path = "live_poller_tests.rs"]
mod tests;
