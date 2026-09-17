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
//! # ⚠️ What is here, and how much of it a test reaches
//!
//! **The retry rule is exercised**, since 16 September 2026: `listener_tests`
//! drives `run` against `test_geyser_server`, a scripted Yellowstone server in the
//! test process. Every ending has a test that goes red when its answer to
//! *which ending restarts the budget, which charges it, and what the next
//! attempt asks for* changes. Until then the rule was read and never run, which
//! is how all five of its defects came to be found in review — and how a sixth,
//! the clean-EOF twin of the `Failed` reset, was still uncovered by the first
//! version of those very tests.
//!
//! The rule is split in two on purpose: the `match` below decides the **retry
//! budget**, and [`Attempt::next_resume_from`] decides the **resume point**. It
//! was one thing in four arms until a seventh defect — a delivered session with
//! no mark of its own erasing the mark the loop held — showed what four
//! identical assignments cost. The eighth was its mirror image: the same
//! expression reading a failure that never reached the server as the server
//! refusing the replay, which is what [`Attempt::Unreachable`] now separates.
//!
//! ⚠️ **That ending is guarded from two sides, and only one of them is `run`.**
//! What it costs the retry budget is driven end to end, against a port with
//! nothing behind it. What it does to the mark cannot be: observing a mark being
//! *kept* needs one to exist first, and only a delivered session makes one —
//! against a server that must, for the same attempt, be unreachable. So the
//! mark half is driven one level down, at `connect_and_stream`, on the value
//! the production return sites produce. The loop's single `resume_from =`
//! assignment is what the five tests asserting on `resume_points` drive.
//!
//! ⚠️ **One decision is deliberately left unguarded: the backoff reset.** Both
//! churn arms put `backoff` back to `INITIAL_BACKOFF_SECS`, and no test
//! observes it, because the only observable is *how long* the next attempt
//! waits. Pinning it means driving the backoff up, cutting the stream, and
//! asserting on an elapsed duration with a tolerance — slow, and exactly the
//! sort of timing assertion that goes red on a loaded runner for reasons that
//! have nothing to do with the rule. Named here rather than covered by a
//! sentence that says "every arm" and means "almost".
//!
//! What no test here reaches is the rest of the file: TLS, the keep-alive, the
//! connect timeout, and — the one that matters — whether a provider honours
//! `from_slot` the way this code assumes. A scripted server validates **this
//! client against our model of the server**, never the protocol;
//! `02 - backlog/[spike]flux-grpc-reel-mesures.md` is where that model meets a
//! real one.
//!
//! The split that made the rule reachable at all still holds: the request is
//! built in `subscription` and the meaning of an update in `session`, so the
//! two facts the retry rules turn on — `StreamSession::received_data` and
//! `StreamSession::resume_from` — are computed and tested next door. What is
//! tested *here* is that the listener does the right thing with them.

use std::{collections::HashSet, error::Error, sync::Arc, time::Duration};

use solana_pubkey::Pubkey;
use tokio::sync::{Mutex, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{
    Status,
    transport::{Channel, ClientTlsConfig, Endpoint as ChannelEndpoint},
};
use tracing::{info, warn};
use yellowstone_grpc_proto::prelude::{SubscribeRequest, geyser_client::GeyserClient};
use yog_bootstrap::Endpoint;
use yog_core::domain::Protocol;

use crate::{
    application::source::IngestedTransaction,
    error::GrpcListenerError,
    infra::{
        Credential,
        endpoint::scheme,
        grpc::{
            interceptor::CredentialInterceptor,
            session::{SessionState, StreamSession},
            subscription::build_request,
        },
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
/// Holds the same watch set as `RpcListener`, for the same reason: an address
/// is an address, a program id or a pool, and which ones go in is decided
/// upstream by the daemon's registration.
pub(crate) struct GrpcListener {
    /// The whole endpoint and not just its URL: on this path the credential can
    /// ride in a metadata header, which is `Endpoint::header`'s half of the
    /// question — see `interceptor`.
    endpoint: Endpoint,
    /// Every address to subscribe to, with its protocol — see
    /// `subscription::build_request`, which groups them into one filter per
    /// protocol.
    watched: Mutex<HashSet<(Protocol, Pubkey)>>,
    max_attempts: u32,
}

impl GrpcListener {
    pub(crate) fn new(endpoint: Endpoint, max_attempts: u32) -> Self {
        Self {
            endpoint,
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
        let credential = Credential::new(self.endpoint.header())?;
        let interceptor = CredentialInterceptor::new(&credential)?;
        let channel = self.channel_endpoint()?;
        let request = self.subscribe_request(None).await?;

        info!(
            endpoint = %self.endpoint,
            header = credential.name().unwrap_or("none"),
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

            let outcome = self
                .connect_and_stream(&channel, &interceptor, request, &downstream, &shutdown)
                .await;

            // ⚠️ **The only place `resume_from` is written**, and that is the
            // point. It was four assignments, one per arm below, all spelled
            // `resume_from = mark` — two meaning *keep* and two meaning
            // *abandon*, told apart only by reading the arm they sat in. One of
            // the two keeping arms then had to learn that an absent mark is not
            // a mark at zero, and nothing would have said if only one of them
            // had. [`Attempt::next_resume_from`] holds the whole rule; the arms
            // below hold the retry budget and nothing else.
            resume_from = outcome.next_resume_from(resume_from);

            match outcome {
                Attempt::ShutdownRequested => {
                    info!("shutdown requested — gRPC listener stopping");
                    return Ok(());
                }
                Attempt::DownstreamClosed => {
                    info!("downstream channel closed — gRPC listener stopping");
                    return Ok(());
                }
                Attempt::StreamClosed {
                    delivered: true, ..
                } => {
                    // The connection lived long enough to deliver. That is churn,
                    // not a failing provider, so the budget starts over — the
                    // same reading `SubscriptionWorker` makes of a closed stream.
                    warn!(attempt, "gRPC stream closed — resubscribing");
                    attempt = 0;
                    backoff = INITIAL_BACKOFF_SECS;
                    sleep_or_cancel(Duration::from_secs(1), &shutdown).await;
                }

                // ⚠️ **A stream that opened and closed having delivered nothing
                // is a failing attempt, not churn**, and the difference is the
                // whole retry budget. `subscribe` succeeding says very little:
                // an exhausted quota, a `from_slot` past the server's retention
                // or a token refused at stream level rather than at the
                // handshake all land here. Resetting the counter on them
                // redials a bandwidth-billed provider once a second for ever —
                // `max_attempts` never reached, no shutdown, one `warn!` per
                // second as the only trace. Found in review, 9 September 2026.
                // `SubscriptionWorker` resets unconditionally because it has no
                // such signal; this path has one.
                Attempt::StreamClosed {
                    delivered: false, ..
                } => {
                    warn!(
                        attempt,
                        max = self.max_attempts,
                        "gRPC stream closed without delivering anything"
                    );

                    if attempt >= self.max_attempts {
                        return Err(GrpcListenerError::RetriesExhausted {
                            attempts: attempt,
                            last_error: "stream closed without delivering anything".to_string(),
                        });
                    }

                    sleep_or_cancel(Duration::from_secs(backoff), &shutdown).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
                }

                // ⚠️ **A session that delivered and then errored is churn too**,
                // and this arm forgot it until 10 September 2026. `Err(Status)`
                // is not the exotic ending: a GOAWAY, an h2 RST_STREAM, a TCP
                // reset or a nightly provider restart all land here, which is
                // the ordinary way a long-lived stream breaks. With the budget
                // charged and never reset, `attempt` climbed across sessions —
                // 1, 2, 3 … — and the tenth nightly restart shut the indexer
                // down having lost nothing and met no failing provider. The
                // JSON-RPC path never had this hole: `ConnectOutcome::Failed`
                // there is only produced *before* the stream is established, so
                // every ending of a live stream resets.
                //
                // ⚠️ What it costs, and it is the same cost the clean-EOF arm
                // pays: a provider that delivers one transaction and then errors
                // every second is retried for ever. `received_data` is what
                // narrows that — a ping does not count — but nothing bounds a
                // server that really does send data before failing. The counter
                // is what would say it.
                Attempt::Failed {
                    error,
                    delivered: true,
                    ..
                } => {
                    warn!(attempt, error = %error, "gRPC stream broke — resubscribing");
                    attempt = 0;
                    backoff = INITIAL_BACKOFF_SECS;
                    sleep_or_cancel(Duration::from_secs(1), &shutdown).await;
                }

                // ⚠️ **Two endings, one budget, on purpose.** An attempt that
                // never reached the service and one the server answered with a
                // refusal cost the same: they are both a failing attempt, and
                // charging one and not the other would redial a dead endpoint
                // for ever. What they do *not* share is the resume point, which
                // is [`Attempt::next_resume_from`]'s and not this arm's — the
                // two questions were one thing until 16 September 2026, and the
                // answer for one was wrong for the other.
                //
                // ⚠️ Which of the two happened is **not** readable from the
                // error text: `subscribe:` prefixes a refused subscription and
                // a connection that gave way during it alike. What the line
                // carries instead is the consequence — the `from_slot` the next
                // attempt will ask for, already decided above. A gap given up
                // is exactly what nothing used to say.
                Attempt::Unreachable { error }
                | Attempt::Failed {
                    error,
                    delivered: false,
                    ..
                } => {
                    warn!(
                        attempt,
                        max = self.max_attempts,
                        error = %error,
                        resume_from = ?resume_from,
                        "gRPC stream attempt failed"
                    );

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

    /// Build the subscription from what is watched right now.
    async fn subscribe_request(
        &self,
        from_slot: Option<u64>,
    ) -> Result<SubscribeRequest, GrpcListenerError> {
        build_request(&*self.watched.lock().await, from_slot)
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
            // Nothing has been asked of anyone yet, and that is the whole
            // difference [`Attempt::Unreachable`] exists to carry.
            Err(e) => {
                return Attempt::Unreachable {
                    error: url.scrub(&format!("connect: {e}")),
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
            return Attempt::Unreachable {
                error: "outbound stream closed before the subscription was sent".to_string(),
            };
        }

        let mut stream = match client.subscribe(ReceiverStream::new(outbound_rx)).await {
            Ok(response) => response.into_inner(),
            // ⚠️ **A `Status` here is not proof that a server spoke.** tonic maps
            // a connection that gave way into a `Status` as well, so this one
            // site carries both a refused subscription and our own transport
            // failing — which is the shape the ~2-minute link cuts actually
            // take, since `connect()` succeeds against a socket cut right after
            // the TCP handshake. Found in review, 16 September 2026, after the
            // first version of this fix had trusted this site.
            Err(status) if !reached_the_service(&status) => {
                return Attempt::Unreachable {
                    error: url.scrub(&format!("subscribe: {status}")),
                };
            }
            Err(status) => {
                return Attempt::Failed {
                    // A `Status` carries the server's message, which can quote
                    // the request — scrubbed like every other third-party string.
                    error: url.scrub(&format!("subscribe: {status}")),
                    delivered: false,
                    resume_from: None,
                };
            }
        };

        info!(
            from_slot = ?request.from_slot,
            "subscribed to the Yellowstone stream"
        );

        let mut session = StreamSession::new(downstream.clone(), outbound_tx, shutdown.clone());

        loop {
            tokio::select! {
                biased;

                _ = shutdown.cancelled() => return Attempt::ShutdownRequested,

                message = stream.message() => match message {
                    Ok(Some(update)) => match session.handle(update).await {
                        SessionState::Open => {}
                        SessionState::DownstreamClosed => return Attempt::DownstreamClosed,
                        // The session was parked on a full consumer when the
                        // token fired. This arm is what makes that wait
                        // interruptible: `handle` runs in the *body* of this
                        // arm, not as a `select!` branch, so nothing here polls
                        // the token while it is inside.
                        SessionState::ShutdownRequested => return Attempt::ShutdownRequested,
                    },
                    Ok(None) => return Attempt::StreamClosed {
                        delivered: session.received_data(),
                        resume_from: session.resume_from(),
                    },
                    Err(status) => return Attempt::Failed {
                        error: url.scrub(&format!("stream: {status}")),
                        delivered: session.received_data(),
                        resume_from: session.resume_from(),
                    },
                },
            }
        }
    }
}

/// How one connection ended.
///
/// The two **stream** endings carry the same two facts, and the listener needs
/// both on both — dropping `delivered` from one of them was a defect of its
/// own, see the `Failed` arm: `resume_from` is where to pick up (see
/// `StreamSession::resume_from`), and `delivered` says whether this attempt got
/// anything off the stream at all, which is what separates the churn of a
/// long-lived connection from a server that accepts a subscription and closes
/// it at once.
///
/// [`Attempt::Unreachable`] carries neither, and that is the point of it: a
/// failure before the service answered has no stream to report on, and no
/// verdict of anyone's to carry.
enum Attempt {
    ShutdownRequested,
    DownstreamClosed,
    /// The attempt never got an answer from the service.
    ///
    /// ⚠️ **Not a variety of `Failed`, and the difference is a lost gap.** No
    /// `from_slot` of ours was ever accepted or refused, so nothing here is a
    /// judgement on the mark we hold — which is all
    /// [`Attempt::next_resume_from`] reads it for. There is no `delivered`
    /// (nothing can have been) and no `resume_from` (no session existed to
    /// compute one).
    ///
    /// Three sites produce it, and the third is the one that hides:
    ///
    /// - **the dial** — `channel.connect()` failing, which is a port with
    ///   nothing behind it, a name that does not resolve, a network that is
    ///   gone;
    /// - **`subscribe` failing on our own transport** — a `Status` tonic built
    ///   from a broken connection rather than one a server sent. This is what
    ///   the ~2-minute link cuts of a laptop actually produce: a socket cut
    ///   just after the TCP handshake still lets `connect()` succeed, because
    ///   hyper's HTTP/2 handshake does not wait for the server's settings. See
    ///   [`reached_the_service`];
    /// - **the outbound half** — which **cannot happen today**: the receiver is
    ///   alive on the next line and the channel has room. The branch is kept
    ///   because it is the honest classification of that `Err`, not because
    ///   anything reaches it.
    Unreachable {
        error: String,
    },
    StreamClosed {
        delivered: bool,
        resume_from: Option<u64>,
    },
    Failed {
        error: String,
        delivered: bool,
        resume_from: Option<u64>,
    },
}

impl Attempt {
    /// Where the next attempt resumes from, given the mark the loop already
    /// `held`.
    ///
    /// The whole resume rule, in one expression, because it used to be four
    /// assignments spelled identically in four arms — two meaning *keep* and two
    /// meaning *abandon*, told apart only by reading the arm around them. A rule
    /// that lives at four sites gets corrected at three.
    ///
    /// # ⚠️ An absent mark is not a mark at zero
    ///
    /// A session can end **delivered with nothing to resume from**: a
    /// transaction matching no protocol filter is counted `Unroutable` and
    /// dropped before the buffer, while `handle` has already recorded that data
    /// came off the stream — rightly, since the server is not refusing us. So a
    /// delivered attempt's mark *completes* the one we hold: what it never
    /// replaces is a mark we hold with **nothing**. A mark it does carry wins,
    /// including one further back — which is the unbounded rewind named at the
    /// end of this comment, and not a second reading of this sentence.
    /// Overwriting it with `None` threw away a still-valid resume point and
    /// sent the attempt after it to the live edge; the transactions of the
    /// original break were then never asked for again, and no event table can
    /// know a row is missing. Found in review of PR #149, 16 September 2026.
    ///
    /// # ⚠️ A mark is given up only to the server that refused it
    ///
    /// A replay can be refused for a reason this code cannot see — the slot may
    /// be past the server's retention, and providers do not agree on how far
    /// back that goes. So a replay the **server** was asked for and did not
    /// honour is not asked for twice: the next attempt starts from the live
    /// edge, losing the gap rather than looping on a request that cannot
    /// succeed. No error string is read to decide it; what decides it is that
    /// the server had the request and gave nothing back.
    ///
    /// `None` states that rule rather than carrying the attempt's own mark,
    /// which is the same value today and says less: an attempt that delivered
    /// nothing fed neither the buffer nor `highest_meta_slot`, so
    /// [`StreamSession::resume_from`] can only answer `None` for it.
    ///
    /// ⚠️ **A failure before contact is not that**, and reading it as that cost
    /// a gap until 16 September 2026. `channel.connect()` failing and the
    /// outbound half failing both return before `client.subscribe` is ever
    /// called: the `from_slot` never left this process, so no verdict on it can
    /// exist. They are [`Attempt::Unreachable`], and the mark we hold survives
    /// them. The distinction is the one the *return sites* already made — it
    /// took a variant to record it, not a provider's error text to parse, which
    /// is what an earlier version of this comment claimed.
    ///
    /// ⚠️ **One more thing this expression does not do: put a floor under the
    /// mark.** `StreamSession::resume_from` prefers the oldest slot still
    /// pending, which on a replay is about the `from_slot` just asked for, minus
    /// `REWIND_SLOTS`. A stream that breaks before the buffer drains therefore
    /// resumes two slots earlier each round, and the churn arm resets the budget
    /// every time — so a provider that accepts and breaks after one transaction
    /// walks `from_slot` backwards without bound, on a connection billed by the
    /// byte. Pre-existing, unchanged here, and named because this is now the one
    /// expression a reader is sent to.
    fn next_resume_from(&self, held: Option<u64>) -> Option<u64> {
        match self {
            Attempt::StreamClosed {
                delivered: true,
                resume_from,
            }
            | Attempt::Failed {
                delivered: true,
                resume_from,
                ..
            } => (*resume_from).or(held),

            Attempt::StreamClosed {
                delivered: false, ..
            }
            | Attempt::Failed {
                delivered: false, ..
            } => None,

            // Nothing was asked of anyone, so nothing was refused: the mark is
            // exactly as good as it was a moment ago.
            Attempt::Unreachable { .. } => held,

            // Neither ending: the loop returns on both, so this answer is never
            // read. Handing back what we hold is the only one that is not a
            // claim about a stream that had none.
            Attempt::ShutdownRequested | Attempt::DownstreamClosed => held,
        }
    }
}

/// Whether a `Status` is the server answering, or this side's transport giving
/// way.
///
/// ⚠️ **Not error-text matching, and that distinction is the whole point.** The
/// chain of causes is walked for a `tonic::transport::Error`, a type that can
/// only exist on *our* side: a status the server sent arrives in the response
/// trailers and is rebuilt from them with **no source at all**. So a transport
/// error anywhere in the chain says the request never got an answer — a load
/// balancer with no backend, a link cut after the TCP handshake, a redial of
/// tonic's own that found nothing.
///
/// The chain is walked rather than the first link tested because where tonic
/// wraps that error is tonic's business and changes between versions; that it
/// is *there* is the fact this reads.
///
/// ⚠️ **This is asked at `subscribe` and nowhere else.** Once the server has
/// answered `subscribe`, it has seen the `from_slot` we sent, and a stream that
/// then breaks having delivered nothing is the case
/// [`Attempt::next_resume_from`]'s undelivered branch is deliberately about.
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
