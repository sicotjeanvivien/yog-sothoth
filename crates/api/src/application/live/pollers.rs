use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::{BoxFuture, FutureExt, join_all};
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_core::domain::{EventTable, LiveEventFeed};

use super::{EventChannel, EventSource, LIVE_CHANNEL_CAPACITY, LivePoller, LiveSource};
use crate::application::{OnPool, PoolPairs};

type Start = Box<dyn FnOnce(CancellationToken) -> BoxFuture<'static, ()> + Send>;

/// Every poller of `/api/live`, run as one task that ends once each has stopped.
/// ⚠️ One read at a time among them: they share the pool the routes are sized on.
pub(crate) struct LivePollers {
    interval: Duration,
    reads: Arc<Semaphore>,
    pairs: Arc<PoolPairs>,
    starts: Vec<Start>,
}

impl LivePollers {
    pub(crate) fn new(interval: Duration, pairs: Arc<PoolPairs>) -> Self {
        Self {
            interval,
            reads: Arc::new(Semaphore::new(1)),
            pairs,
            starts: Vec::new(),
        }
    }

    pub(crate) fn add<S: LiveSource>(
        &mut self,
        source: Arc<S>,
        sender: broadcast::Sender<Arc<S::Item>>,
    ) {
        let poller = LivePoller::new(source, sender, self.interval, self.reads.clone());
        self.starts.push(Box::new(move |shutdown| {
            poller.run(shutdown).map(|_| ()).boxed()
        }));
    }

    /// A collection's broadcast, with one poller per table writing into it.
    pub(crate) fn add_collection<T: EventTable, E: OnPool + Send + Sync + 'static>(
        &mut self,
        feed: Arc<dyn LiveEventFeed<T, E>>,
    ) -> EventChannel<T, E> {
        let (sender, _) = broadcast::channel(LIVE_CHANNEL_CAPACITY * T::ALL.len());
        for &table in T::ALL {
            self.add(
                Arc::new(EventSource::new(feed.clone(), table, self.pairs.clone())),
                sender.clone(),
            );
        }
        sender
    }

    /// Runs them all until `shutdown`. The `Result` is for [`yog_bootstrap::Stop::settle`].
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        join_all(self.starts.into_iter().map(|start| start(shutdown.clone()))).await;
        Ok(())
    }
}

#[cfg(test)]
#[path = "pollers_tests.rs"]
mod tests;
