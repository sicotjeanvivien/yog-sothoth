//! A Yellowstone server that does exactly what a test tells it to.
//!
//! `GrpcListener::run` decides what to do when a stream ends, and every one of
//! its decisions needs a stream that *ended in a particular way*. Nothing short
//! of a server produces those: the endings are `Ok(None)` versus `Err(Status)`
//! on a live stream, a `Status` at the moment of `subscribe`, and the presence
//! or absence of data before any of them. So the server is here, in the
//! process, scripted.
//!
//! # What it proves and what it does not
//!
//! It validates **our client against our model of the server**, not the
//! protocol. The script is written from a reading of the proto, exactly like
//! the hand-built messages of `test_fixtures`. A provider that honours `from_slot`
//! differently, or pings on a schedule of its own, is
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` — the two are complements,
//! not substitutes.
//!
//! What it *does* prove is the part that has never had a guard: given an
//! ending, the listener charges or restarts the retry budget, resumes from the
//! right slot, and stops when it is told to.
//!
//! # Why it can be plaintext
//!
//! `GrpcListener::channel_endpoint` configures TLS unconditionally, but tonic
//! applies it only to an `https://` URI — `is_https` is a scheme comparison in
//! its connector, so an `http://127.0.0.1:<port>` endpoint connects in the
//! clear through the very same code path production uses. Nothing here is a
//! test-only branch of the listener.
//!
//! # ⚠️ An exhausted script is an error, never a quiet close
//!
//! [`ScriptedSession`] entries are consumed one per `subscribe`. When they run
//! out, this server refuses loudly rather than closing, so a listener that
//! reconnects more times than the test scripted **fails that test** instead of
//! quietly ending on a stream that happened to close. A guard whose own failure
//! mode is silence is the defect these tests exist to catch.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use tokio::{net::TcpListener, sync::mpsc, task::JoinHandle};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::{Request, Response, Status, Streaming, transport::Server};
use yellowstone_grpc_proto::prelude::{
    GetBlockHeightRequest, GetBlockHeightResponse, GetLatestBlockhashRequest,
    GetLatestBlockhashResponse, GetSlotRequest, GetSlotResponse, GetVersionRequest,
    GetVersionResponse, IsBlockhashValidRequest, IsBlockhashValidResponse, PingRequest,
    PongResponse, SubscribeDeshredRequest, SubscribeGossipRequest, SubscribeReplayInfoRequest,
    SubscribeReplayInfoResponse, SubscribeRequest, SubscribeUpdate, SubscribeUpdateDeshred,
    SubscribeUpdateGossip,
    geyser_server::{Geyser, GeyserServer},
};

/// How many updates may queue on one scripted stream. The scripts are a handful
/// of messages long; this only has to be bigger than the longest one so that
/// writing it never blocks on a client that has not read yet.
const STREAM_CAPACITY: usize = 16;

/// One thing the server does on an accepted stream.
pub(super) enum Action {
    /// Put one update on the stream — built by [`Action::send`].
    ///
    /// Boxed because `SubscribeUpdate` is a large protobuf enum and the other
    /// two variants are a `Status` and nothing at all, which clippy reads, with
    /// reason, as an enum whose size is one variant's.
    Send(Box<SubscribeUpdate>),
    /// End the stream with an error — a GOAWAY, an h2 reset, a provider
    /// restart. This is the ordinary way a long-lived stream breaks, which is
    /// why the listener has a whole arm for it.
    Fail(Status),
    /// Keep the stream open and send nothing more, until the client goes away.
    ///
    /// The only way to reach the listener's *other* shutdown path: the one
    /// where the `select!` is parked on `stream.message()` rather than inside
    /// `StreamSession::handle`.
    Hold,
}

impl Action {
    /// Put one update on the stream.
    pub(super) fn send(update: SubscribeUpdate) -> Self {
        Self::Send(Box::new(update))
    }
}

/// What the server does with one `subscribe` call.
pub(super) enum ScriptedSession {
    /// Refuse the subscription itself, **after** reading the request — which is
    /// what makes the refused-`from_slot` case reachable, since the test has to
    /// see what was asked for before it was turned down.
    Refuse(Status),
    /// Accept, play these actions, then close cleanly unless an [`Action`]
    /// ended the stream first.
    Stream(Vec<Action>),
}

impl ScriptedSession {
    /// Accept and close at once, having sent nothing.
    ///
    /// The ending the whole `delivered` distinction exists for: an exhausted
    /// quota or a token refused at stream level looks exactly like this, and
    /// reading it as churn redials a bandwidth-billed provider for ever.
    pub(super) fn closes_empty() -> Self {
        Self::Stream(Vec::new())
    }
}

struct ScriptedGeyser {
    /// Consumed one entry per `subscribe`.
    script: Mutex<VecDeque<ScriptedSession>>,
    /// Every `SubscribeRequest` this server was sent, in order. The test reads
    /// it back to assert what `from_slot` each attempt asked for.
    requests: Arc<Mutex<Vec<SubscribeRequest>>>,
}

#[tonic::async_trait]
impl Geyser for ScriptedGeyser {
    type SubscribeStream = ReceiverStream<Result<SubscribeUpdate, Status>>;

    async fn subscribe(
        &self,
        request: Request<Streaming<SubscribeRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        // Read the subscription before doing anything with it: a test asserts
        // on what each attempt asked for, including the attempts that are
        // refused.
        let subscription = request
            .into_inner()
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("no subscription was sent"))?;

        // Scoped so that no lock is held across an await — the trait requires
        // `Send` futures.
        let scripted = {
            self.requests
                .lock()
                .expect("the recorder is never poisoned")
                .push(subscription);
            self.script
                .lock()
                .expect("the script is never poisoned")
                .pop_front()
        };

        let actions = match scripted {
            Some(ScriptedSession::Refuse(status)) => return Err(status),
            Some(ScriptedSession::Stream(actions)) => actions,
            // See the module header: silence here would let a listener that
            // reconnects too often pass.
            None => {
                return Err(Status::failed_precondition(
                    "the script is exhausted — the listener subscribed more \
                     times than this test scripted",
                ));
            }
        };

        let (tx, rx) = mpsc::channel(STREAM_CAPACITY);
        tokio::spawn(async move {
            for action in actions {
                match action {
                    Action::Send(update) => {
                        if tx.send(Ok(*update)).await.is_err() {
                            return;
                        }
                    }
                    Action::Fail(status) => {
                        let _ = tx.send(Err(status)).await;
                        return;
                    }
                    // Holding `tx` is what keeps the stream open; the task ends
                    // when the server is dropped at the end of the test.
                    Action::Hold => std::future::pending::<()>().await,
                }
            }
            // Falling out of the loop drops `tx`, which the client sees as a
            // clean end of stream — `Ok(None)`.
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    // ── the rest of the service, which this path never calls ────────────
    //
    // `GrpcListener` issues exactly one of the schema's ten RPCs, so the nine
    // stubs below exist only because the trait is the whole schema. A panic is
    // the right answer for them: if one is ever reached, the listener started
    // doing something this file knows nothing about, and that should stop a
    // test rather than return a plausible-looking default.

    type SubscribeDeshredStream = ReceiverStream<Result<SubscribeUpdateDeshred, Status>>;
    type SubscribeGossipStream = ReceiverStream<Result<SubscribeUpdateGossip, Status>>;

    async fn subscribe_deshred(
        &self,
        _request: Request<Streaming<SubscribeDeshredRequest>>,
    ) -> Result<Response<Self::SubscribeDeshredStream>, Status> {
        unimplemented!("the listener does not call SubscribeDeshred")
    }

    async fn subscribe_gossip(
        &self,
        _request: Request<SubscribeGossipRequest>,
    ) -> Result<Response<Self::SubscribeGossipStream>, Status> {
        unimplemented!("the listener does not call SubscribeGossip")
    }

    async fn subscribe_replay_info(
        &self,
        _request: Request<SubscribeReplayInfoRequest>,
    ) -> Result<Response<SubscribeReplayInfoResponse>, Status> {
        unimplemented!("the listener does not call SubscribeReplayInfo")
    }

    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PongResponse>, Status> {
        unimplemented!("the listener never pings: keep-alive is HTTP/2's job")
    }

    async fn get_latest_blockhash(
        &self,
        _request: Request<GetLatestBlockhashRequest>,
    ) -> Result<Response<GetLatestBlockhashResponse>, Status> {
        unimplemented!("the listener does not call GetLatestBlockhash")
    }

    async fn get_block_height(
        &self,
        _request: Request<GetBlockHeightRequest>,
    ) -> Result<Response<GetBlockHeightResponse>, Status> {
        unimplemented!("the listener does not call GetBlockHeight")
    }

    async fn get_slot(
        &self,
        _request: Request<GetSlotRequest>,
    ) -> Result<Response<GetSlotResponse>, Status> {
        unimplemented!("the listener does not call GetSlot")
    }

    async fn is_blockhash_valid(
        &self,
        _request: Request<IsBlockhashValidRequest>,
    ) -> Result<Response<IsBlockhashValidResponse>, Status> {
        unimplemented!("the listener does not call IsBlockhashValid")
    }

    async fn get_version(
        &self,
        _request: Request<GetVersionRequest>,
    ) -> Result<Response<GetVersionResponse>, Status> {
        unimplemented!("the listener does not call GetVersion")
    }
}

/// A running scripted server, and the way back to what it was asked.
pub(super) struct ScriptedGeyserHandle {
    /// The plaintext URL to hand `Endpoint::for_tests`.
    url: String,
    requests: Arc<Mutex<Vec<SubscribeRequest>>>,
    server: JoinHandle<()>,
}

impl ScriptedGeyserHandle {
    pub(super) fn url(&self) -> &str {
        &self.url
    }

    /// Every subscription this server was sent, in order.
    ///
    /// Its **length** is the number of connection attempts the listener made,
    /// and each entry's `from_slot` is where that attempt asked to resume —
    /// the two facts every retry test turns on.
    pub(super) fn requests(&self) -> Vec<SubscribeRequest> {
        self.requests
            .lock()
            .expect("the recorder is never poisoned")
            .clone()
    }

    /// The `from_slot` of each attempt, in order — the readable form of
    /// [`Self::requests`] for the tests that only care about resumption.
    pub(super) fn resume_points(&self) -> Vec<Option<u64>> {
        self.requests()
            .iter()
            .map(|request| request.from_slot)
            .collect()
    }
}

impl Drop for ScriptedGeyserHandle {
    fn drop(&mut self) {
        // The server outlives nothing: a test that ended must not leave a
        // listener socket behind for the next one.
        self.server.abort();
    }
}

/// Bind a scripted server on a free port and start serving.
pub(super) async fn start(script: Vec<ScriptedSession>) -> ScriptedGeyserHandle {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free loopback port");
    let address = listener.local_addr().expect("a bound address");

    let requests = Arc::new(Mutex::new(Vec::new()));
    let service = ScriptedGeyser {
        script: Mutex::new(script.into()),
        requests: Arc::clone(&requests),
    };

    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(GeyserServer::new(service))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .expect("the scripted server serves until the test drops it");
    });

    ScriptedGeyserHandle {
        // ⚠️ `http://`, and the module header says why that is not a shortcut.
        url: format!("http://{address}"),
        requests,
        server,
    }
}
