//! What opens one connection per attempt, and follows it to the end of its stream.

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

use yog_bootstrap::Endpoint;

use super::{ending::Ending, log, stall_clock::StallClock};

/// How long to wait for the TCP+TLS handshake before calling an attempt failed.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP/2 keep-alive, also while idle: a stream nothing matches looks idle to a load balancer.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// Ceiling on one decoded message.
/// ⚠️ tonic's 4 MiB default is too small for a busy block's meta, and its refusal ends the stream
/// on an error that looks like a network fault.
const MAX_DECODING_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// How many outbound requests may queue: the subscription, then one answer per
/// server ping. A full channel means the transport is not draining; the
/// session then drops the answer rather than wait.
const OUTBOUND_CAPACITY: usize = 8;

type Client = GeyserClient<InterceptedService<Channel, CredentialInterceptor>>;

/// A stream the server has accepted, and what must live as long as it.
struct OpenStream {
    /// Held, never read: nothing here should depend on what tonic does once the last client is gone.
    _client: Client,
    stream: Streaming<SubscribeUpdate>,
    /// The outbound half, handed to the session for its ping answers.
    outbound: mpsc::Sender<SubscribeRequest>,
}

/// Opens a connection per attempt, and follows it to the end of its stream.
pub(super) struct Connector {
    /// The whole endpoint and not just its URL: on this path the credential can
    /// ride in a metadata header — see `interceptor`.
    endpoint: Endpoint,
    /// [`STALL_TIMEOUT`](super::stall_clock::STALL_TIMEOUT) in production; a parameter so a test
    /// can stall a stream in milliseconds.
    stall_timeout: Duration,
}

impl Connector {
    pub(super) fn new(endpoint: Endpoint, stall_timeout: Duration) -> Self {
        Self {
            endpoint,
            stall_timeout,
        }
    }

    pub(super) fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// The address to dial, built once; TLS applies only to `https://`, so `http://` connects in the clear.
    pub(super) fn channel_endpoint(&self) -> Result<ChannelEndpoint, GrpcListenerError> {
        let url = self.endpoint.url();

        // ⚠️ Every error here goes through `SecretUrl::scrub`: tonic quotes the URI, which carries
        // the key when the operator put it there rather than in a header.
        let endpoint = ChannelEndpoint::from_shared(url.expose().to_string()).map_err(|e| {
            GrpcListenerError::InvalidEndpoint {
                reason: url.scrub(&e.to_string()),
            }
        })?;

        // ⚠️ `from_shared` ignores the scheme, so a `wss://` URL would fail in the retry loop;
        // checked after it, so a URI tonic cannot read keeps tonic's own reason.
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

    /// One attempt: open a stream, then read it to its end.
    pub(super) async fn attempt(
        &self,
        channel: &ChannelEndpoint,
        interceptor: &CredentialInterceptor,
        request: SubscribeRequest,
        downstream: &mpsc::Sender<IngestedTransaction>,
        shutdown: &CancellationToken,
    ) -> Ending {
        match self.open(channel, interceptor, request, shutdown).await {
            Ok(open) => self.read(open, downstream, shutdown).await,
            Err(ending) => ending,
        }
    }

    /// Dial, send the subscription, and wait for the server to accept it.
    async fn open(
        &self,
        channel: &ChannelEndpoint,
        interceptor: &CredentialInterceptor,
        request: SubscribeRequest,
        shutdown: &CancellationToken,
    ) -> Result<OpenStream, Ending> {
        let channel: Channel = channel.connect().await.map_err(|e| {
            // Nothing has been asked of anyone yet.
            Ending::Unreachable {
                error: self.endpoint.url().scrub(&format!("connect: {e}")),
            }
        })?;

        let mut client = GeyserClient::with_interceptor(channel, interceptor.clone())
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE);

        // The outbound half stays open for the life of the stream: the session
        // answers server pings on it — see `PingAnswer`.
        let (outbound, outbound_rx) = mpsc::channel::<SubscribeRequest>(OUTBOUND_CAPACITY);
        if outbound.send(request.clone()).await.is_err() {
            return Err(Ending::Unreachable {
                error: "outbound stream closed before the subscription was sent".to_string(),
            });
        }

        let stream = self.subscribe(&mut client, outbound_rx, shutdown).await?;
        log::subscribed(request.from_slot);

        Ok(OpenStream {
            _client: client,
            stream,
            outbound,
        })
    }

    /// Read an open stream until it ends, stalls, or the listener stops.
    async fn read(
        &self,
        open: OpenStream,
        downstream: &mpsc::Sender<IngestedTransaction>,
        shutdown: &CancellationToken,
    ) -> Ending {
        let OpenStream {
            _client,
            mut stream,
            outbound,
        } = open;
        let mut session = StreamSession::new(downstream.clone(), outbound, shutdown.clone());
        let mut clock = StallClock::new(self.stall_timeout);

        loop {
            clock.wait_started(Instant::now());

            tokio::select! {
                biased;

                _ = shutdown.cancelled() => return Ending::ShutdownRequested,

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
    /// ⚠️ Bounded by the stall timeout, since a server may never send headers, and interruptible.
    async fn subscribe(
        &self,
        client: &mut Client,
        outbound_rx: mpsc::Receiver<SubscribeRequest>,
        shutdown: &CancellationToken,
    ) -> Result<Streaming<SubscribeUpdate>, Ending> {
        let subscribed = tokio::select! {
            biased;

            _ = shutdown.cancelled() => return Err(Ending::ShutdownRequested),

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
            // ⚠️ A `Status` is not proof a server spoke: tonic builds one when our transport gives
            // way. Asked only here: once `subscribe` is answered, the server has seen `from_slot`.
            Err(status) if !reached_the_service(&status) => Err(Ending::Unreachable {
                error: url.scrub(&format!("subscribe: {status}")),
            }),
            // A `Status` carries the server's message, which can quote the
            // request — scrubbed like every other third-party string.
            Err(status) => Err(Ending::Failed {
                error: url.scrub(&format!("subscribe: {status}")),
                code: status.code(),
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
    ) -> Option<Ending> {
        match message {
            Ok(Some(update)) => {
                let block_metas = session.block_metas_received();
                let ending = ending_of(session.handle(update).await);
                if session.block_metas_received() != block_metas {
                    clock.heard_block_meta();
                }
                ending
            }
            Ok(None) => Some(Ending::StreamClosed {
                delivered: session.received_data(),
                resume_from: session.resume_from(),
            }),
            Err(status) => Some(Ending::Failed {
                error: self.endpoint.url().scrub(&format!("stream: {status}")),
                code: status.code(),
                delivered: session.received_data(),
                resume_from: session.resume_from(),
            }),
        }
    }

    /// End an attempt as [`Ending::Stalled`], counted by site. Not logged
    /// here: the budget logs every ending once, with the mark it will use.
    fn stalled(&self, site: StallSite, delivered: bool, resume_from: Option<u64>) -> Ending {
        GrpcListenerMetrics::record_stall(site);

        Ending::Stalled {
            error: format!("stalled: {} for {:?}", site.describe(), self.stall_timeout),
            delivered,
            resume_from,
        }
    }
}

/// The ending a session's state brings, if any.
fn ending_of(state: SessionState) -> Option<Ending> {
    match state {
        SessionState::Open => None,
        SessionState::DownstreamClosed => Some(Ending::DownstreamClosed),
        SessionState::ShutdownRequested => Some(Ending::ShutdownRequested),
    }
}

/// Whether a `Status` is the server answering, or our transport giving way.
/// ⚠️ Walks the causes for a [`tonic::transport::Error`], which only our side builds; not the first
/// link only, since where tonic wraps it changes between versions.
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
