//! A Yellowstone server that does exactly what a test tells it to: our client against our model
//! of the server, never the protocol. Plaintext reaches the production path, tonic applying TLS
//! to `https://` only.
//! ⚠️ An exhausted script refuses loudly rather than closing, so a listener that reconnects more
//! often than scripted fails its test.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
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

use crate::infra::grpc::test_fixtures::ping;

/// How many updates may queue on one scripted stream: more than the longest script.
const STREAM_CAPACITY: usize = 16;

/// One thing the server does on an accepted stream.
pub(super) enum Action {
    /// Put one update on the stream, built by [`Action::send`]; boxed, `SubscribeUpdate` dwarfs
    /// the other variants.
    Send(Box<SubscribeUpdate>),
    /// End the stream with an error: a GOAWAY, an h2 reset, a provider restart.
    Fail(Status),
    /// Keep the stream open and send nothing more, until the client goes away: the `select!`
    /// parked on `stream.message()`.
    Hold,
    /// Wait this long before the next action: a stream slow and still alive.
    Wait(Duration),
    /// Ping at this interval and send nothing else: a stopped data path whose ping task runs on.
    PingEvery(Duration),
}

impl Action {
    /// Put one update on the stream.
    pub(super) fn send(update: SubscribeUpdate) -> Self {
        Self::Send(Box::new(update))
    }
}

/// What the server does with one `subscribe` call.
pub(super) enum ScriptedSession {
    /// Refuse the subscription itself, after reading the request, so the test sees what was asked.
    Refuse(Status),
    /// Accept, play these actions, then close cleanly unless an [`Action`] ended the stream first.
    Stream(Vec<Action>),
    /// Read the request and never answer it: a wedged backend behind a front end.
    NeverAnswer,
}

impl ScriptedSession {
    /// Accept and close at once, having sent nothing: an exhausted quota looks exactly like this.
    pub(super) fn closes_empty() -> Self {
        Self::Stream(Vec::new())
    }
}

struct ScriptedGeyser {
    /// Consumed one entry per `subscribe`.
    script: Mutex<VecDeque<ScriptedSession>>,
    /// Every `SubscribeRequest` this server was sent, in order.
    requests: Arc<Mutex<Vec<SubscribeRequest>>>,
}

#[tonic::async_trait]
impl Geyser for ScriptedGeyser {
    type SubscribeStream = ReceiverStream<Result<SubscribeUpdate, Status>>;

    async fn subscribe(
        &self,
        request: Request<Streaming<SubscribeRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        // Read before anything else: a test asserts on what each attempt asked, refused ones too.
        let subscription = request
            .into_inner()
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("no subscription was sent"))?;

        // Scoped: no lock is held across an await, the trait requires `Send` futures.
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
            Some(ScriptedSession::NeverAnswer) => std::future::pending().await,
            Some(ScriptedSession::Stream(actions)) => actions,
            // Loud, or a listener that reconnects too often would pass.
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
                    // Holding `tx` keeps the stream open until the server is dropped.
                    Action::Hold => std::future::pending::<()>().await,
                    Action::Wait(duration) => tokio::time::sleep(duration).await,
                    // A send fails only once the client has gone, and the stream ends with it.
                    Action::PingEvery(interval) => loop {
                        tokio::time::sleep(interval).await;
                        if tx.send(Ok(ping())).await.is_err() {
                            return;
                        }
                    },
                }
            }
            // Dropping `tx` is a clean end of stream, `Ok(None)` for the client.
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    // ⚠️ The listener calls one RPC of ten: a stub reached means it does something this file
    // knows nothing about, and a panic stops the test rather than a plausible default.

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
        unimplemented!("the listener answers stream pings and never calls the unary one")
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

    /// Every subscription this server was sent, in order: its length is the number of attempts.
    pub(super) fn requests(&self) -> Vec<SubscribeRequest> {
        self.requests
            .lock()
            .expect("the recorder is never poisoned")
            .clone()
    }

    /// The `from_slot` of each attempt, in order.
    pub(super) fn resume_points(&self) -> Vec<Option<u64>> {
        self.requests()
            .iter()
            .map(|request| request.from_slot)
            .collect()
    }
}

impl Drop for ScriptedGeyserHandle {
    fn drop(&mut self) {
        // A finished test must not leave a listening socket behind.
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
        url: format!("http://{address}"),
        requests,
        server,
    }
}
