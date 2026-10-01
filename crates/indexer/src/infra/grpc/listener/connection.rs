//! One connection, from the dial to the end of its stream.

use std::{error::Error, time::Duration};

use tokio::{sync::mpsc, time::Instant};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{
    Status, Streaming,
    service::interceptor::InterceptedService,
    transport::{Channel, ClientTlsConfig, Endpoint as ChannelEndpoint},
};
use yellowstone_grpc_proto::prelude::{
    SubscribeRequest, SubscribeUpdate, geyser_client::GeyserClient,
};

use crate::{
    application::source::IngestedTransaction,
    error::GrpcListenerError,
    infra::{
        endpoint::scheme,
        grpc::{
            interceptor::CredentialInterceptor,
            metrics::{GrpcListenerMetrics, StallSite},
            session::{SessionState, StreamSession},
        },
    },
};

use super::{GrpcListener, ending::Attempt, log, stall_clock::StallClock};

/// How long to wait for the TCP+TLS handshake before calling an attempt failed.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP/2 keep-alive. An idle connection is what a load balancer collects, and
/// a Geyser stream is idle whenever nothing matches the filters;
/// `keep_alive_while_idle` keeps the path warm through those stretches.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// Ceiling on one decoded message.
///
/// ⚠️ Not a tuning knob: tonic's 4 MiB default is too small for a busy
/// block's meta or a long log, and the refusal ends the stream on an error
/// that looks like a network fault. The buffer downstream bounds memory.
const MAX_DECODING_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// How many outbound requests may queue: the subscription, then one answer per
/// server ping. A full channel means the transport is not draining; the
/// session then drops the answer rather than wait.
const OUTBOUND_CAPACITY: usize = 8;

type Client = GeyserClient<InterceptedService<Channel, CredentialInterceptor>>;

impl GrpcListener {
    /// The address to dial, built once.
    ///
    /// TLS is configured unconditionally and applies only to an `https://`
    /// URL — tonic reads the scheme, so a self-hosted `http://` Yellowstone
    /// connects in the clear without a second code path.
    pub(super) fn channel_endpoint(&self) -> Result<ChannelEndpoint, GrpcListenerError> {
        let url = self.endpoint.url();

        // ⚠️ Every error out of this function goes through `SecretUrl::scrub`:
        // `tonic` quotes the URI it was handed, and that URI carries the key
        // whenever the operator put it there rather than in a header.
        let endpoint = ChannelEndpoint::from_shared(url.expose().to_string()).map_err(|e| {
            GrpcListenerError::InvalidEndpoint {
                reason: url.scrub(&e.to_string()),
            }
        })?;

        // ⚠️ `from_shared` does not look at the scheme, so a `wss://` URL would
        // otherwise fail inside the retry loop — see `scheme::GRPC`. Checked
        // after it, so that a URI tonic cannot read keeps tonic's own reason.
        scheme::check(&url, &scheme::GRPC)
            .map_err(|reason| GrpcListenerError::InvalidEndpoint { reason })?;

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

    /// One connection: dial, subscribe, then read until the stream ends.
    pub(super) async fn connect_and_stream(
        &self,
        channel: &ChannelEndpoint,
        interceptor: &CredentialInterceptor,
        request: SubscribeRequest,
        downstream: &mpsc::Sender<IngestedTransaction>,
        shutdown: &CancellationToken,
    ) -> Attempt {
        let channel: Channel = match channel.connect().await {
            Ok(channel) => channel,
            // Nothing has been asked of anyone yet.
            Err(e) => {
                return Attempt::Unreachable {
                    error: self.endpoint.url().scrub(&format!("connect: {e}")),
                };
            }
        };

        let client = GeyserClient::with_interceptor(channel, interceptor.clone())
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE);

        // The outbound half stays open for the life of the stream: the session
        // answers server pings on it — see `PingAnswer`.
        let (outbound_tx, outbound_rx) = mpsc::channel::<SubscribeRequest>(OUTBOUND_CAPACITY);
        if outbound_tx.send(request.clone()).await.is_err() {
            return Attempt::Unreachable {
                error: "outbound stream closed before the subscription was sent".to_string(),
            };
        }

        let mut stream = match self.subscribe(client, outbound_rx, shutdown).await {
            Ok(stream) => stream,
            Err(ending) => return ending,
        };

        log::subscribed(request.from_slot);

        let mut session = StreamSession::new(downstream.clone(), outbound_tx, shutdown.clone());
        let mut clock = StallClock::new(self.stall_timeout);

        loop {
            clock.wait_started(Instant::now());

            tokio::select! {
                biased;

                _ = shutdown.cancelled() => return Attempt::ShutdownRequested,

                message = stream.message() => {
                    clock.wait_ended(Instant::now());
                    if let Some(ending) = self.on_message(message, &mut session, &mut clock).await {
                        return ending;
                    }
                },

                // After `message`, so that a message already waiting — which
                // may be the block-meta that resets the clock — is read first.
                _ = tokio::time::sleep(clock.remaining()) => {
                    return self.stalled(
                        StallSite::Stream,
                        session.received_data(),
                        session.resume_from(),
                    );
                }
            }
        }
    }

    /// Send the subscription and wait for the server to answer it.
    ///
    /// ⚠️ Bounded by the stall timeout, or a server that never sends response
    /// headers hangs the listener here; and interruptible, since that timeout
    /// outlasts a container's stop grace period.
    async fn subscribe(
        &self,
        mut client: Client,
        outbound_rx: mpsc::Receiver<SubscribeRequest>,
        shutdown: &CancellationToken,
    ) -> Result<Streaming<SubscribeUpdate>, Attempt> {
        let subscribed = tokio::select! {
            biased;

            _ = shutdown.cancelled() => return Err(Attempt::ShutdownRequested),

            subscribed = tokio::time::timeout(
                self.stall_timeout,
                client.subscribe(ReceiverStream::new(outbound_rx)),
            ) => subscribed,
        };
        let Ok(subscribed) = subscribed else {
            return Err(self.stalled(StallSite::Subscribe, false, None));
        };

        let url = self.endpoint.url();
        match subscribed {
            Ok(response) => Ok(response.into_inner()),
            // ⚠️ A `Status` here is not proof that a server spoke: tonic also
            // builds one from our own transport giving way, which is what a
            // link cut just after the TCP handshake produces.
            Err(status) if !reached_the_service(&status) => Err(Attempt::Unreachable {
                error: url.scrub(&format!("subscribe: {status}")),
            }),
            // A `Status` carries the server's message, which can quote the
            // request — scrubbed like every other third-party string.
            Err(status) => Err(Attempt::Failed {
                error: url.scrub(&format!("subscribe: {status}")),
                delivered: false,
                resume_from: None,
            }),
        }
    }

    /// What one message off the stream does, and the ending it brings, if any.
    async fn on_message(
        &self,
        message: Result<Option<SubscribeUpdate>, Status>,
        session: &mut StreamSession,
        clock: &mut StallClock,
    ) -> Option<Attempt> {
        match message {
            Ok(Some(update)) => {
                let block_metas = session.block_metas_received();
                match session.handle(update).await {
                    SessionState::Open => {}
                    SessionState::DownstreamClosed => return Some(Attempt::DownstreamClosed),
                    // The session was parked on a full consumer when the token
                    // fired: `handle` runs in the body of the `select!` arm, so
                    // nothing else polled the token meanwhile.
                    SessionState::ShutdownRequested => return Some(Attempt::ShutdownRequested),
                }
                if session.block_metas_received() != block_metas {
                    clock.heard_block_meta();
                }
                None
            }
            Ok(None) => Some(Attempt::StreamClosed {
                delivered: session.received_data(),
                resume_from: session.resume_from(),
            }),
            Err(status) => Some(Attempt::Failed {
                error: self.endpoint.url().scrub(&format!("stream: {status}")),
                delivered: session.received_data(),
                resume_from: session.resume_from(),
            }),
        }
    }

    /// End an attempt as [`Attempt::Stalled`], counted by site. Not logged
    /// here: the budget logs every ending once, with the mark it will use.
    fn stalled(&self, site: StallSite, delivered: bool, resume_from: Option<u64>) -> Attempt {
        GrpcListenerMetrics::record_stall(site);

        Attempt::Stalled {
            error: format!("stalled: {} for {:?}", site.describe(), self.stall_timeout),
            delivered,
            resume_from,
        }
    }
}

/// Whether a `Status` is the server answering, or this side's transport giving
/// way.
///
/// ⚠️ Not error-text matching: the chain of causes is walked for a
/// [`tonic::transport::Error`], a type that exists only on *our* side — a
/// status the server sent is rebuilt from the trailers with no source at all.
/// Walked rather than tested at the first link, because where tonic wraps it
/// changes between versions.
///
/// ⚠️ Asked at `subscribe` and nowhere else: once `subscribe` is answered, the
/// server has seen our `from_slot`.
fn reached_the_service(status: &Status) -> bool {
    let mut source = status.source();

    while let Some(error) = source {
        if error.is::<tonic::transport::Error>() {
            return false;
        }
        source = error.source();
    }

    true
}
