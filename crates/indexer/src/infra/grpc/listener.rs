//! The Yellowstone connection: one stream, and what to do when it breaks.
//!
//! Sibling of [`crate::infra::rpc::RpcListener`], and **not** its translation:
//! no fleet of workers (one `SubscribeRequest` describes everything), no
//! `TransactionFetcher` (the transaction arrives whole, with its `index` in the
//! block), and no `SignatureDispatcher` (failed transactions are refused by the
//! server; the invocation filter has no server-side equivalent — see
//! `subscription`).
//!
//! This file assembles. Each responsibility has its own module:
//!
//! - `connector` — opens one connection per attempt, and follows it to the
//!   end of its stream;
//! - `ending` — how it ended, what that is worth (its `Verdict`), and what
//!   that does to the resume mark;
//! - `retry_budget` — whether to try again, and when;
//! - `stall_clock` — how long the server has been silent;
//! - `log` — the lines the listener writes.
//!
//! # ⚠️ What a test reaches
//!
//! `listener_tests` drives `run` against `test_geyser_server`, a scripted
//! Yellowstone server in the test process: every ending has a test that goes
//! red when its answer — restart or charge the budget, what the next attempt
//! asks for — changes. The budget and the stall clock are also tested on their
//! own values, with no clock involved.
//!
//! ⚠️ The mark half of [`Ending::Unreachable`](ending::Ending::Unreachable) is driven at
//! `connect_and_stream`, not `run`: observing a mark *kept* needs a delivered
//! session first, against a server that must then be unreachable.
//!
//! What no local test reaches: TLS, the keep-alive, the connect timeout,
//! whether a provider takes the answer to its pings, and whether it honours
//! `from_slot` the way this code assumes. A scripted server validates this
//! client against **our model** of the server, never the protocol.

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

/// Subscribes to a Yellowstone stream and turns it into timestamped
/// transactions.
///
/// Holds the same watch set as `RpcListener`, for the same reason: an address
/// is an address, a program id or a pool, and which ones go in is decided
/// upstream by the daemon's registration.
pub(crate) struct GrpcListener {
    /// What opens a connection per attempt — the endpoint and the stall
    /// timeout.
    connector: Connector,
    /// Every address to subscribe to, with its protocol — see
    /// [`subscription::build_request`], which groups them into one filter per
    /// protocol.
    ///
    /// [`subscription::build_request`]: crate::infra::grpc::subscription::build_request
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

    /// Open the stream, keep it open, and return when it is over.
    ///
    /// Returns `Ok(())` on a requested shutdown or when the consumer has gone,
    /// and `Err` when the retry budget runs out or the configuration cannot
    /// produce a subscription at all.
    ///
    /// # The three failures that are not retried
    ///
    /// A bad header, an unusable URL and an empty subscription are all
    /// configuration, and a configuration does not repair itself between two
    /// attempts. They fail before the loop, so an operator sees the cause
    /// rather than ten backoffs and a budget exhausted.
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
                .connect_and_stream(&channel, &interceptor, request, &downstream, &shutdown)
                .await
                .verdict();

            // ⚠️ The only place `resume_from` is written. The mark rule and the
            // budget both read the one verdict, and neither hides in the other.
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
