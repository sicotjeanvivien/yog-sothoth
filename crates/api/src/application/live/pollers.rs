use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::{BoxFuture, FutureExt, join_all};
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_core::domain::{EventTable, LiveEventFeed};

use super::{EventSource, LIVE_CHANNEL_CAPACITY, LiveEvent, LivePoller, LiveSource};

type Start = Box<dyn FnOnce(CancellationToken) -> BoxFuture<'static, ()> + Send>;

/// Every poller of `/api/live`, run as one task that ends once each has stopped.
/// ⚠️ One read at a time among them: they share the pool the routes are sized on.
pub(crate) struct LivePollers {
    interval: Duration,
    reads: Arc<Semaphore>,
    starts: Vec<Start>,
}

impl LivePollers {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            reads: Arc::new(Semaphore::new(1)),
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
    pub(crate) fn add_collection<T: EventTable, E: Send + Sync + 'static>(
        &mut self,
        feed: Arc<dyn LiveEventFeed<T, E>>,
    ) -> broadcast::Sender<Arc<LiveEvent<T, E>>> {
        let (sender, _) = broadcast::channel(LIVE_CHANNEL_CAPACITY * T::ALL.len());
        for &table in T::ALL {
            self.add(
                Arc::new(EventSource::new(feed.clone(), table)),
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
