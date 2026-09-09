//! The Yellowstone connection: one stream, and what to do when it breaks.
//!
//! Sibling of [`crate::infra::rpc::RpcListener`], and **not** its translation.
//! Three things it does not have, all for the same reason — Yellowstone
//! delivers where `logsSubscribe` notifies:
//!
//! - **no fleet of workers.** `SubscriptionWorker` exists because
//!   `logsSubscribe` accepts exactly one pubkey per subscription, so watching
//!   *n* addresses means *n* WebSockets. One `SubscribeRequest` describes
//!   everything here, so there is one connection and no supervision problem;
//! - **no `TransactionFetcher` downstream.** The transaction arrives whole, with
//!   its `index` in the block — which is the ambiguity, measured at 17.8 % of
//!   current-state updates, that this whole change is about;
//! - **no `SignatureDispatcher`.** Its two filters are gone or moved: failed
//!   transactions are refused by the server (`failed: Some(false)`), and the
//!   invocation filter has no server-side equivalent — see `subscription`.
//!
//! # ⚠️ What is here and cannot be tested here
//!
//! Everything in this file that touches the network. No gRPC endpoint is
//! reachable without a subscription, so the connection, TLS, the retry budget,
//! keep-alive and the exact semantics of `from_slot` are **written, reviewed and
//! unproven**. `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` is where they
//! meet a server. What *is* testable was deliberately moved out: the request
//! into `subscription`, the meaning of each update into `session`.

use std::{collections::HashSet, sync::Arc, time::Duration};

use solana_pubkey::Pubkey;
use tokio::sync::{Mutex, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint as ChannelEndpoint};
use tracing::{info, warn};
use yellowstone_grpc_proto::prelude::{SubscribeRequest, geyser_client::GeyserClient};
use yog_bootstrap::Endpoint;
use yog_core::domain::Protocol;

use crate::{
    bootstrap::IngestScope,
    error::GrpcListenerError,
    infra::grpc::{
        ingested_transaction::IngestedTransaction,
        interceptor::CredentialInterceptor,
        session::{SessionState, StreamSession},
        subscription::build_request,
    },
};

/// Retry budget shape, shared with `SubscriptionWorker` — the same provider is
/// on the other end, and an operator reading two different backoffs would have
/// to learn two.
const INITIAL_BACKOFF_SECS: u64 = 1;
const MAX_BACKOFF_SECS: u64 = 60;

/// How long to wait for the TCP+TLS handshake before calling an attempt failed.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP/2 keep-alive. A Geyser stream is idle for as long as nothing matches
/// the filters, and an idle connection is what a load balancer between us and
/// the validator collects. Sending a PING frame keeps the path warm, and
/// `keep_alive_while_idle` is what makes that true during the quiet stretches
/// that are precisely the risk.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// Ceiling on one decoded message.
///
/// ⚠️ **Not a tuning knob — tonic's default is 4 MiB and it is too small.** A
/// busy block's meta, or a transaction with a long log, goes past it; the
/// message is then refused mid-stream and the connection ends on an error that
/// looks like a network fault and is a limit. 64 MiB is generous on purpose:
/// the buffer downstream, not this, is what bounds memory.
const MAX_DECODING_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// How many outbound requests may queue. One subscription plus the occasional
/// ping answer — anything above a handful means the outbound half is stuck, and
/// queueing more keep-alives would not unstick it.
const OUTBOUND_CAPACITY: usize = 8;

/// Subscribes to a Yellowstone stream and turns it into timestamped
/// transactions.
///
/// Holds the same two watch sets as `RpcListener`, for the same reason: what is
/// subscribed to is decided by `INGEST_SCOPE`, and the pool set is restored
/// from the database at startup.
pub(crate) struct GrpcListener {
    /// The whole endpoint and not just its URL: on this path the credential can
    /// ride in a metadata header, which is `Endpoint::header`'s half of the
    /// question — see `interceptor`.
    endpoint: Endpoint,
    watched_protocols: Mutex<HashSet<Protocol>>,
    watched_pools: Mutex<HashSet<(Protocol, Pubkey)>>,
    max_attempts: u32,
    scope: IngestScope,
}

impl GrpcListener {
    pub(crate) fn new(endpoint: Endpoint, max_attempts: u32, scope: IngestScope) -> Self {
        Self {
            endpoint,
            watched_protocols: Mutex::new(HashSet::new()),
            watched_pools: Mutex::new(HashSet::new()),
            max_attempts,
            scope,
        }
    }

    pub(crate) async fn watch(&self, protocol: Protocol) {
        self.watched_protocols.lock().await.insert(protocol);
    }

    pub(crate) async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.watched_pools
            .lock()
            .await
            .insert((protocol, pool_address));
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
        let interceptor = CredentialInterceptor::new(self.endpoint.header())?;
        let channel = self.channel_endpoint()?;
        let request = self.subscribe_request(None).await?;

        info!(
            endpoint = %self.endpoint,
            header = interceptor.header_name().unwrap_or("none"),
            filters = request.transactions.len(),
            accounts = request
                .transactions
                .values()
                .map(|f| f.account_include.len())
                .sum::<usize>(),
            "gRPC listener starting"
        );

        let mut attempt: u32 = 0;
        let mut backoff = INITIAL_BACKOFF_SECS;
        let mut resume_from: Option<u64> = None;

        loop {
            if shutdown.is_cancelled() {
                info!("shutdown requested — gRPC listener stopping");
                return Ok(());
            }

            attempt += 1;
            let request = self.subscribe_request(resume_from).await?;

            match self
                .connect_and_stream(&channel, &interceptor, request, &downstream, &shutdown)
                .await
            {
                Attempt::ShutdownRequested => {
                    info!("shutdown requested — gRPC listener stopping");
                    return Ok(());
                }
                Attempt::DownstreamClosed => {
                    info!("downstream channel closed — gRPC listener stopping");
                    return Ok(());
                }
                Attempt::StreamClosed { highest_slot } => {
                    // The connection lived long enough to deliver. That is churn,
                    // not a failing provider, so the budget starts over — the
                    // same reading `SubscriptionWorker` makes of a closed stream.
                    warn!(attempt, "gRPC stream closed — resubscribing");
                    attempt = 0;
                    backoff = INITIAL_BACKOFF_SECS;
                    resume_from = highest_slot.map(|slot| slot + 1);
                    sleep_or_cancel(Duration::from_secs(1), &shutdown).await;
                }
                Attempt::Failed {
                    error,
                    highest_slot,
                } => {
                    warn!(
                        attempt,
                        max = self.max_attempts,
                        error = %error,
                        "gRPC stream attempt failed"
                    );

                    // ⚠️ The `from_slot` fallback, and it is deterministic on
                    // purpose. A replay can be refused for a reason this code
                    // cannot see — the slot may be past the server's retention,
                    // and providers do not agree on how far back that goes. So a
                    // replay that never established is not asked for twice:
                    // the next attempt starts from the live edge, losing the
                    // gap rather than looping on a request that cannot succeed.
                    // No error string is read to decide this; only whether the
                    // stream produced anything.
                    resume_from = highest_slot.map(|slot| slot + 1);

                    if attempt >= self.max_attempts {
                        return Err(GrpcListenerError::RetriesExhausted {
                            attempts: attempt,
                            last_error: error,
                        });
                    }

                    sleep_or_cancel(Duration::from_secs(backoff), &shutdown).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
                }
            }
        }
    }

    /// The address to dial, built once.
    ///
    /// TLS is configured unconditionally and applies only to an `https://`
    /// URL — tonic reads the scheme, so a self-hosted `http://` Yellowstone
    /// connects in the clear without a second code path here.
    fn channel_endpoint(&self) -> Result<ChannelEndpoint, GrpcListenerError> {
        let url = self.endpoint.url();

        // ⚠️ Every error out of this function goes through `SecretUrl::scrub`:
        // `tonic` quotes the URI it was handed, and that URI carries the key
        // whenever the operator put it there rather than in a header.
        let endpoint = ChannelEndpoint::from_shared(url.expose().to_string()).map_err(|e| {
            GrpcListenerError::InvalidEndpoint {
                reason: url.scrub(&e.to_string()),
            }
        })?;

        endpoint
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .map(|endpoint| {
                endpoint
                    .connect_timeout(CONNECT_TIMEOUT)
                    .http2_keep_alive_interval(KEEPALIVE_INTERVAL)
                    .keep_alive_while_idle(true)
            })
            .map_err(|e| GrpcListenerError::InvalidEndpoint {
                reason: url.scrub(&e.to_string()),
            })
    }

    /// Build the subscription from what is watched right now.
    async fn subscribe_request(
        &self,
        from_slot: Option<u64>,
    ) -> Result<SubscribeRequest, GrpcListenerError> {
        build_request(
            self.scope,
            &*self.watched_protocols.lock().await,
            &*self.watched_pools.lock().await,
            from_slot,
        )
    }

    /// One connection, from dial to close.
    async fn connect_and_stream(
        &self,
        channel: &ChannelEndpoint,
        interceptor: &CredentialInterceptor,
        request: SubscribeRequest,
        downstream: &mpsc::Sender<IngestedTransaction>,
        shutdown: &CancellationToken,
    ) -> Attempt {
        let url = self.endpoint.url();

        let channel: Channel = match channel.connect().await {
            Ok(channel) => channel,
            Err(e) => {
                return Attempt::Failed {
                    error: url.scrub(&format!("connect: {e}")),
                    highest_slot: None,
                };
            }
        };

        let mut client = GeyserClient::with_interceptor(channel, interceptor.clone())
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE);

        // The outbound half stays open for the life of the stream. Half-closing
        // it after the subscription would be legal HTTP/2, but it also removes
        // the only way to answer a ping — and a client that never speaks is what
        // an idle-timing middlebox collects.
        let (outbound_tx, outbound_rx) = mpsc::channel::<SubscribeRequest>(OUTBOUND_CAPACITY);
        if outbound_tx.send(request.clone()).await.is_err() {
            return Attempt::Failed {
                error: "outbound stream closed before the subscription was sent".to_string(),
                highest_slot: None,
            };
        }

        let mut stream = match client.subscribe(ReceiverStream::new(outbound_rx)).await {
            Ok(response) => response.into_inner(),
            Err(status) => {
                return Attempt::Failed {
                    // A `Status` carries the server's message, which can quote
                    // the request — scrubbed like every other third-party string.
                    error: url.scrub(&format!("subscribe: {status}")),
                    highest_slot: None,
                };
            }
        };

        info!(
            from_slot = ?request.from_slot,
            "subscribed to the Yellowstone stream"
        );

        let mut session = StreamSession::new(downstream.clone(), outbound_tx, request);

        loop {
            tokio::select! {
                biased;

                _ = shutdown.cancelled() => return Attempt::ShutdownRequested,

                message = stream.message() => match message {
                    Ok(Some(update)) => {
                        if session.handle(update).await == SessionState::DownstreamClosed {
                            return Attempt::DownstreamClosed;
                        }
                    }
                    Ok(None) => return Attempt::StreamClosed {
                        highest_slot: session.highest_slot(),
                    },
                    Err(status) => return Attempt::Failed {
                        error: url.scrub(&format!("stream: {status}")),
                        highest_slot: session.highest_slot(),
                    },
                },
            }
        }
    }
}

/// How one connection ended.
///
/// `highest_slot` is what a resubscription resumes from, and it is `None`
/// exactly when the attempt produced nothing — which is also what tells the
/// caller a replay was refused rather than exhausted.
enum Attempt {
    ShutdownRequested,
    DownstreamClosed,
    StreamClosed {
        highest_slot: Option<u64>,
    },
    Failed {
        error: String,
        highest_slot: Option<u64>,
    },
}

/// Sleep, unless the shutdown token fires first.
async fn sleep_or_cancel(duration: Duration, shutdown: &CancellationToken) {
    tokio::select! {
        _ = tokio::time::sleep(duration) => {}
        _ = shutdown.cancelled() => {}
    }
}
