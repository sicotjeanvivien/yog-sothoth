//! The Yellowstone connection: one stream, and what to do when it breaks. `connector` makes one
//! attempt, `ending` judges how it ended, `retry_budget` decides whether to try again.
//! ⚠️ `listener_tests` drives `run` against a scripted server: it proves this client against our
//! model of the server, never TLS, keep-alive, ping answers or `from_slot` on a real provider.

mod connector;
mod ending;
mod log;
mod retry_budget;
mod stall_clock;

use std::{collections::HashSet, sync::Arc, time::Duration};

use solana_pubkey::Pubkey;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use yellowstone_grpc_proto::prelude::SubscribeRequest;
use yog_bootstrap::Endpoint;
use yog_core::domain::Protocol;

use crate::{
    application::source::IngestedTransaction,
    error::GrpcListenerError,
    infra::{
        Credential,
        grpc::{interceptor::CredentialInterceptor, subscription::build_request},
    },
};

use connector::Connector;
use retry_budget::{Next, RetryBudget};

pub(crate) use stall_clock::STALL_TIMEOUT;

/// Subscribes to a Yellowstone stream and turns it into timestamped transactions; what it
/// watches, program ids or pools, is decided upstream by the daemon.
pub(crate) struct GrpcListener {
    /// What opens a connection per attempt: the endpoint and the stall timeout.
    connector: Connector,
    /// Every address to subscribe to, with its protocol.
    watched: Mutex<HashSet<(Protocol, Pubkey)>>,
    max_attempts: u32,
}

impl GrpcListener {
    pub(crate) fn new(endpoint: Endpoint, max_attempts: u32, stall_timeout: Duration) -> Self {
        Self {
            connector: Connector::new(endpoint, stall_timeout),
            watched: Mutex::new(HashSet::new()),
            max_attempts,
        }
    }

    /// Watch a whole protocol: its program id goes into the filter.
    pub(crate) async fn watch(&self, protocol: Protocol) {
        self.watched
            .lock()
            .await
            .insert((protocol, protocol.program_id()));
    }

    pub(crate) async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.watched.lock().await.insert((protocol, pool_address));
    }

    /// Open the stream, keep it open, and return `Ok(())` on shutdown or a departed consumer.
    /// `Err` when the retry budget runs out, or before the loop on a configuration fault (bad
    /// header, unusable URL, empty subscription), which no retry would repair.
    pub(crate) async fn run(
        self: Arc<Self>,
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Result<(), GrpcListenerError> {
        let credential = Credential::new(self.connector.endpoint().header())?;
        let interceptor = CredentialInterceptor::new(&credential)?;
        let channel = self.connector.channel_endpoint()?;
        let request = self.subscribe_request(None).await?;

        log::starting(self.connector.endpoint(), credential.name(), &request);

        let mut budget = RetryBudget::new(self.max_attempts);
        let mut resume_from: Option<u64> = None;

        loop {
            if shutdown.is_cancelled() {
                log::stopping_on_shutdown();
                return Ok(());
            }

            let request = self.subscribe_request(resume_from).await?;

            let verdict = self
                .connector
                .attempt(&channel, &interceptor, request, &downstream, &shutdown)
                .await
                .verdict();

            // ⚠️ The only place `resume_from` is written; the mark rule and the budget read the one verdict.
            resume_from = verdict.next_resume_from(resume_from);

            match budget.settle(verdict, resume_from) {
                Next::Retry { after } => sleep_or_cancel(after, &shutdown).await,
                Next::Stop(result) => return result,
            }
        }
    }

    /// Build the subscription from what is watched right now.
    async fn subscribe_request(
        &self,
        from_slot: Option<u64>,
    ) -> Result<SubscribeRequest, GrpcListenerError> {
        build_request(&*self.watched.lock().await, from_slot)
    }
}

/// Sleep, unless the shutdown token fires first.
async fn sleep_or_cancel(duration: Duration, shutdown: &CancellationToken) {
    tokio::select! {
        _ = tokio::time::sleep(duration) => {}
        _ = shutdown.cancelled() => {}
    }
}

#[cfg(test)]
#[path = "tests/listener_tests.rs"]
mod tests;
