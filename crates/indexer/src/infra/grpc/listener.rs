//! The Yellowstone connection: one stream, and what to do when it breaks. `connector` makes one
//! attempt, `ending` judges how it ended, `retry_budget` decides whether to try again.
//! ⚠️ `listener_tests` drives `run` against a scripted server: it proves this client against our
//! model of the server, never TLS, keep-alive, ping answers or `from_slot` on a real provider.

mod connector;
mod ending;
mod log;
mod retry_budget;
mod slots_recorder;
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
        grpc::{
            interceptor::CredentialInterceptor,
            metrics::{GrpcListenerMetrics, WAITING_PEAKS},
            ranges_to_record::RangesToRecord,
            session::received_slots::ReceivedSlots,
            subscription::build_request,
        },
    },
};

use connector::Connector;
use retry_budget::{Next, RetryBudget};

pub(crate) use slots_recorder::{RECORD_PERIOD, SlotsRecorder};
pub(crate) use stall_clock::STALL_TIMEOUT;

/// Subscribes to a Yellowstone stream and turns it into timestamped transactions; what it
/// watches, program ids or pools, is decided upstream by the daemon.
pub(crate) struct GrpcListener {
    /// What opens a connection per attempt: the endpoint and the stall timeout.
    connector: Connector,
    /// Every address to subscribe to, with its protocol.
    watched: Mutex<HashSet<(Protocol, Pubkey)>>,
    max_attempts: u32,
    /// What writes the slots the sessions received; `None` writes nothing.
    recorder: Option<SlotsRecorder>,
}

impl GrpcListener {
    pub(crate) fn new(endpoint: Endpoint, max_attempts: u32, stall_timeout: Duration) -> Self {
        Self {
            connector: Connector::new(endpoint, stall_timeout),
            watched: Mutex::new(HashSet::new()),
            max_attempts,
            recorder: None,
        }
    }

    /// Write the slots each session receives for good, through `recorder`.
    pub(crate) fn with_slots_recorder(mut self, recorder: SlotsRecorder) -> Self {
        self.recorder = Some(recorder);
        self
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
        WAITING_PEAKS.export();

        let ranges = RangesToRecord::default();
        let recorder_stop = shutdown.child_token();
        let recording = async {
            if let Some(recorder) = self.recorder.clone() {
                recorder.run(ranges.clone(), recorder_stop.clone()).await;
            }
        };
        let attempts = async {
            let result = self
                .attempts(&channel, &interceptor, &downstream, &ranges, &shutdown)
                .await;
            recorder_stop.cancel();
            result
        };

        // ⚠️ In this task, never spawned: its panic is the source's and stops the indexer.
        let (result, ()) = tokio::join!(attempts, recording);
        result
    }

    /// Connect, read, and reconnect until the budget or the shutdown says stop.
    async fn attempts(
        &self,
        channel: &tonic::transport::Endpoint,
        interceptor: &CredentialInterceptor,
        downstream: &mpsc::Sender<IngestedTransaction>,
        ranges: &RangesToRecord,
        shutdown: &CancellationToken,
    ) -> Result<(), GrpcListenerError> {
        let mut budget = RetryBudget::new(self.max_attempts);
        let mut resume_from: Option<u64> = None;

        loop {
            if shutdown.is_cancelled() {
                log::stopping_on_shutdown();
                return Ok(());
            }

            let (request, protocols) = self.subscription(resume_from).await?;
            let received_slots = ReceivedSlots::new(protocols, ranges.clone());

            let verdict = self
                .connector
                .attempt(
                    channel,
                    interceptor,
                    request,
                    downstream,
                    received_slots,
                    shutdown,
                )
                .await
                .verdict();

            if let Some((slot, reason)) = verdict.abandoned_mark(resume_from) {
                log::mark_abandoned(slot, reason);
                GrpcListenerMetrics::record_mark_abandoned(reason);
            }
            // ⚠️ The only place `resume_from` is written; the mark rule and the budget read the one verdict.
            resume_from = verdict.next_resume_from(resume_from);

            match budget.settle(verdict, resume_from) {
                Next::Retry { after } => sleep_or_cancel(after, shutdown).await,
                Next::Stop(result) => return result,
            }
        }
    }

    /// The subscription, and the protocols it watches whole, read under one lock so they agree.
    async fn subscription(
        &self,
        from_slot: Option<u64>,
    ) -> Result<(SubscribeRequest, Arc<[Protocol]>), GrpcListenerError> {
        let watched = self.watched.lock().await;
        Ok((
            build_request(&watched, from_slot)?,
            whole_protocols(&watched),
        ))
    }

    /// Build the subscription from what is watched right now.
    async fn subscribe_request(
        &self,
        from_slot: Option<u64>,
    ) -> Result<SubscribeRequest, GrpcListenerError> {
        build_request(&*self.watched.lock().await, from_slot)
    }
}

/// The protocols watched whole, by program id: a pool watched alone does not cover its protocol.
fn whole_protocols(watched: &HashSet<(Protocol, Pubkey)>) -> Arc<[Protocol]> {
    watched
        .iter()
        .filter(|(protocol, address)| *address == protocol.program_id())
        .map(|(protocol, _)| *protocol)
        .collect()
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
