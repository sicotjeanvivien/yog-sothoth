//! The listener before its loop (endpoint and watch set), then the retry rules inside it, driven
//! against `test_geyser_server`: this client against our model of the server, not a real one.

use super::*;

use super::ending::Ending;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> GrpcListener {
    GrpcListener::new(Endpoint::for_tests(url, None), 1, STALL_TIMEOUT)
}

/// Accepted means built: a plaintext `http://` endpoint also survives `tls_config`.
#[test]
fn an_http_endpoint_is_accepted() {
    for url in [
        "https://grpc.example.com:443",
        "http://127.0.0.1:10000",
        "HTTPS://x.io",
    ] {
        assert!(
            listener(url).connector.channel_endpoint().is_ok(),
            "{url} is a gRPC endpoint"
        );
    }
}

/// Mutation this is written against: deleting the scheme check from `channel_endpoint`
/// (`scheme_tests` would stay green).
#[test]
fn a_websocket_endpoint_is_refused_before_the_loop() {
    let detail = listener("wss://api.example.com")
        .connector
        .channel_endpoint()
        .expect_err("a WebSocket URL is not a gRPC endpoint")
        .to_string();

    // The scheme check's own wording, not merely `InvalidEndpoint`, which `from_shared` and
    // `tls_config` raise too.
    assert!(detail.contains("`wss` scheme"), "{detail}");
}

/// ⚠️ `grpc.example.com:443` is a valid URI, so only the scheme check can name the mistake.
#[test]
fn a_url_without_a_scheme_reaches_the_scheme_check() {
    let detail = listener("grpc.example.com:443")
        .connector
        .channel_endpoint()
        .expect_err("a schemeless URL is not a gRPC endpoint")
        .to_string();

    assert!(detail.contains("no scheme"), "{detail}");
}

/// Mutation this is written against: `watch` inserting anything but the program id, which would
/// open a stream that matches nothing.
#[tokio::test]
async fn a_protocol_is_watched_through_its_program_id_and_a_pool_as_itself() {
    let listener = listener("https://grpc.example.com:443");
    let pool = Pubkey::new_from_array([7; 32]);

    listener.watch(Protocol::MeteoraDammV2).await;
    listener.watch_pool(Protocol::MeteoraDammV2, pool).await;

    let request = listener
        .subscribe_request(None)
        .await
        .expect("two addresses are watched");

    let mut expected = vec![
        Protocol::MeteoraDammV2.program_id().to_string(),
        pool.to_string(),
    ];
    expected.sort();
    assert_eq!(
        request.transactions["meteora_damm_v2"].account_include,
        expected
    );
}

// ── the retry rules, against a scripted server ──────────────────────
//
// ⚠️ One entanglement: a mark given up or kept needs one first, so breaking the errored stream's
// mark reddens more tests than its owner.

use tokio::{net::TcpListener, time::timeout};
use tonic::Status;

use crate::infra::grpc::{
    test_fixtures::{
        PROTOCOL, block_meta, ping, slot_finalized, transaction, unroutable_transaction,
    },
    test_geyser_server::{self, Action, ScriptedGeyserHandle, ScriptedSession},
};

/// Bounds the failure mode of every mutation below, a listener retrying for ever: without it the
/// suite would hang instead of going red.
const TEST_DEADLINE: Duration = Duration::from_secs(30);

/// Any block time: a block-meta without one gives its slot up instead of releasing it.
const BLOCK_TIME: i64 = 1_700_000_000;

/// A listener pointed at a scripted server, watching DAMM v2, with the production stall timeout.
/// ⚠️ The `watch` is needed: an empty subscription is refused before the retry loop.
async fn listener_for(server: &ScriptedGeyserHandle, max_attempts: u32) -> Arc<GrpcListener> {
    listener_with_stall(server, max_attempts, STALL_TIMEOUT).await
}

/// The same, with a stall timeout a test can wait through.
async fn listener_with_stall(
    server: &ScriptedGeyserHandle,
    max_attempts: u32,
    stall_timeout: Duration,
) -> Arc<GrpcListener> {
    let listener = Arc::new(GrpcListener::new(
        Endpoint::for_tests(server.url(), None),
        max_attempts,
        stall_timeout,
    ));
    listener.watch(Protocol::MeteoraDammV2).await;
    listener
}

/// A transaction of `slot`, carrying the filter name that makes it routable.
/// ⚠️ A name that is no protocol drops it before the buffer, and the resumption tests assert nothing.
fn routable(slot: u64) -> Action {
    Action::send(transaction(slot, &[PROTOCOL.as_str()]))
}

/// A transaction of `slot` the pipeline cannot route: delivered, with nothing to resume from.
fn unroutable(slot: u64) -> Action {
    Action::send(unroutable_transaction(slot))
}

/// A `http://` URL nothing listens on, the only way to a refused dial in this process; a port
/// rebound in between would fail the test on the ending it got, not pass it.
async fn a_closed_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free loopback port");
    let address = listener.local_addr().expect("a bound address");
    drop(listener);

    format!("http://{address}")
}

/// A port that accepts every connection and cuts it before any HTTP/2 byte.
/// ⚠️ Not a refused dial: `connect()` succeeds, so the fault surfaces inside `subscribe` as a `Status`.
struct CuttingPort {
    url: String,
    accepting: tokio::task::JoinHandle<()>,
}

impl CuttingPort {
    async fn bind() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free loopback port");
        let address = listener.local_addr().expect("a bound address");

        // Every socket: tonic redials underneath a failing RPC, and a served one would succeed.
        let accepting = tokio::spawn(async move {
            while let Ok((connection, _)) = listener.accept().await {
                drop(connection);
            }
        });

        Self {
            url: format!("http://{address}"),
            accepting,
        }
    }
}

impl Drop for CuttingPort {
    fn drop(&mut self) {
        self.accepting.abort();
    }
}

/// Wait until the server has been subscribed to at least `count` times.
async fn wait_for_subscriptions(server: &ScriptedGeyserHandle, count: usize) {
    let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
    while server.requests().len() < count {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the listener never subscribed {count} time(s); it subscribed {}",
            server.requests().len()
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// ⚠️ A server pings right after `subscribe`, so this is the ordinary shape of a refusal.
/// Mutation this is written against: `session.rs`'s `Ping` arm setting `received_data = true`;
/// the number of attempts says it, the error stays the same.
#[tokio::test]
async fn a_stream_that_only_pings_before_closing_spends_the_budget() {
    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(vec![Action::send(ping())]),
        ScriptedSession::Stream(vec![Action::send(ping())]),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 2)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("a budget of two must run out rather than retry for ever");

    assert!(
        matches!(
            outcome,
            Err(GrpcListenerError::RetriesExhausted { attempts: 2, .. })
        ),
        "a keep-alive is not delivery, so both attempts are charged: {outcome:?}"
    );
    assert_eq!(
        server.requests().len(),
        2,
        "exactly the budget, and no more — a ping counted as data would restart \
         it and redial for ever"
    );
}

/// The arithmetic is the assertion: delivers then errors, closes empty, closes empty, with a
/// budget of two, is three subscriptions; charged, it would be two.
/// Mutation this is written against: `Ending::verdict` classing `Failed { delivered: true }` as `Unanswered`.
#[tokio::test]
async fn a_stream_that_delivered_before_breaking_restarts_the_budget() {
    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the provider restarted")),
        ]),
        ScriptedSession::closes_empty(),
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 2)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("the two empty closes must exhaust the restarted budget");

    assert!(
        matches!(
            outcome,
            Err(GrpcListenerError::RetriesExhausted { attempts: 2, .. })
        ),
        "the budget runs out on the two empty closes, not before: {outcome:?}"
    );
    assert_eq!(
        server.requests().len(),
        3,
        "the session that delivered must not be charged: two attempts after it, \
         not one"
    );
}

/// ⚠️ A TCP reset and a GOAWAY cut the same block in the same place: the error arm carries the mark too.
/// Mutation this is written against: `on_message`'s `Err(status)` ending handing back
/// `resume_from: None` instead of `session.resume_from()`.
#[tokio::test]
async fn an_error_mid_block_resumes_from_the_slot_that_was_cut() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction, then the stream breaks with the slot still open.
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the connection was reset")),
        ]),
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 1)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("one empty close after the churn exhausts a budget of one");

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(
        server.resume_points(),
        vec![None, Some(8)],
        "a stream that broke mid-block must ask for slot 10 again, rewound by \
         two — exactly like one that closed there"
    );
}

/// ⚠️ A drained node and an idle timeout end as `Ok(None)`: churn, like the error arm.
/// Mutation this is written against: `Ending::verdict` classing `StreamClosed { delivered: true }`
/// as `Unanswered` instead of `Delivered`.
#[tokio::test]
async fn a_stream_that_delivered_before_closing_cleanly_restarts_the_budget() {
    let server = test_geyser_server::start(vec![
        // No `Fail`: the actions run out, a clean end of stream.
        ScriptedSession::Stream(vec![routable(10)]),
        ScriptedSession::closes_empty(),
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 2)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("the two empty closes must exhaust the restarted budget");

    assert!(
        matches!(
            outcome,
            Err(GrpcListenerError::RetriesExhausted { attempts: 2, .. })
        ),
        "the budget runs out on the two empty closes, not before: {outcome:?}"
    );
    assert_eq!(
        server.requests().len(),
        3,
        "a clean close after delivering is churn, not a failing attempt: two \
         attempts after it, not one"
    );
}

/// ⚠️ The buffer dies with the session, so slot 10 without its block-meta is asked for again.
/// Mutation this is written against: `on_message`'s `Ok(None)` ending handing back
/// `resume_from: None` instead of `session.resume_from()`.
#[tokio::test]
async fn a_break_mid_block_resumes_from_the_slot_that_was_cut() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction, then the stream ends with the slot still open.
        ScriptedSession::Stream(vec![routable(10)]),
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 1)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("one empty close after the churn exhausts a budget of one");

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(
        server.resume_points(),
        vec![None, Some(8)],
        "the second attempt must ask for slot 10 again, rewound by two — asking \
         for 11 would drop exactly what the break destroyed"
    );
}

/// ⚠️ An absent mark is not zero: an unroutable delivery leaves `resume_from() == None`.
/// Mutation this is written against: `next_resume_from`'s `Delivered` arm returning `*mark`
/// instead of `mark.or(held)`; the first attempt's mark is scaffolding.
#[tokio::test]
async fn an_attempt_with_nothing_to_resume_from_keeps_the_mark_we_hold() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction and then a break: the mark is 10 rewound by two.
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the connection was reset")),
        ]),
        // Delivery the pipeline cannot route, then a clean close: no mark of its own.
        ScriptedSession::Stream(vec![unroutable(20)]),
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 1)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("one empty close after two churn endings exhausts a budget of one");

    assert!(outcome.is_err(), "{outcome:?}");
    // The whole vector is safe here: no budget rule changes these three resume points.
    assert_eq!(
        server.resume_points(),
        vec![None, Some(8), Some(8)],
        "the unroutable transaction gave that session nothing to resume from; \
         it must not take slot 8 away from the attempt after it"
    );
}

/// A session that delivers slot 10 and breaks: the mark is 8 when the next attempt starts.
fn leaves_the_mark_at_8() -> ScriptedSession {
    ScriptedSession::Stream(vec![
        routable(10),
        Action::Fail(Status::unavailable("the provider restarted")),
    ])
}

/// The `from_slot` the attempt after `ending` asks for, the mark being 8 when `ending` begins.
async fn mark_after(ending: ScriptedSession) -> Option<u64> {
    let server = test_geyser_server::start(vec![
        leaves_the_mark_at_8(),
        ending,
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 2)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("two attempts that deliver nothing exhaust a budget of two");

    assert!(outcome.is_err(), "{outcome:?}");
    let resume_points = server.resume_points();
    assert_eq!(
        resume_points.get(..2),
        Some([None, Some(8)].as_slice()),
        "scaffolding: the ending under test must start with the mark at 8"
    );
    assert_eq!(resume_points.len(), 3, "{resume_points:?}");
    resume_points[2]
}

fn refuses_subscribe(status: Status) -> ScriptedSession {
    ScriptedSession::Refuse(status)
}

/// Accepts `subscribe`, then ends the stream with `status` before sending anything.
fn fails_before_delivering(status: Status) -> ScriptedSession {
    ScriptedSession::Stream(vec![Action::Fail(status)])
}

// Mutation the next five are written against: `refusal_of` answering `Some` for every code.

#[tokio::test]
async fn an_expired_key_keeps_the_mark() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::unauthenticated(
            "the key expired"
        )))
        .await,
        Some(8),
        "a credential says nothing of slot 8: the attempt after asks for it again"
    );
}

#[tokio::test]
async fn a_denied_permission_keeps_the_mark() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::permission_denied(
            "not on this plan"
        )))
        .await,
        Some(8)
    );
}

#[tokio::test]
async fn an_exhausted_quota_keeps_the_mark() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::resource_exhausted(
            "quota reached"
        )))
        .await,
        Some(8)
    );
}

#[tokio::test]
async fn an_unavailable_server_keeps_the_mark() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::unavailable("draining"))).await,
        Some(8)
    );
}

#[tokio::test]
async fn a_provider_incident_keeps_the_mark() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::internal("backend error"))).await,
        Some(8)
    );
}

// Mutation the next three are written against: `refusal_of` answering `None` for every code.

/// ⚠️ The slot may be past retention: the next attempt starts live, never decided on error text.
#[tokio::test]
async fn an_invalid_argument_gives_the_mark_up() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::invalid_argument("bad from_slot"))).await,
        None,
        "the request itself was refused: asking for slot 8 again would be refused again"
    );
}

#[tokio::test]
async fn an_out_of_range_gives_the_mark_up() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::out_of_range("slot 8 is gone"))).await,
        None
    );
}

#[tokio::test]
async fn a_not_found_gives_the_mark_up() {
    assert_eq!(
        mark_after(refuses_subscribe(Status::not_found("slot 8 is gone"))).await,
        None
    );
}

/// ⚠️ The reference server refuses a slot past retention here, on an accepted stream.
/// Mutation this is written against: `on_message`'s `Err(status)` arm losing `code`'s meaning,
/// e.g. passing a fixed `Code::Internal`.
#[tokio::test]
async fn a_stream_failing_out_of_range_before_delivering_gives_the_mark_up() {
    assert_eq!(
        mark_after(fails_before_delivering(Status::out_of_range(
            "broadcast from 8 is not available, last available: 9"
        )))
        .await,
        None
    );
}

/// Mutation this is written against: `on_message`'s `Err(status)` arm passing a fixed refusal
/// code, e.g. `Code::InvalidArgument`.
#[tokio::test]
async fn a_stream_failing_internal_before_delivering_keeps_the_mark() {
    assert_eq!(
        mark_after(fails_before_delivering(Status::internal("backend error"))).await,
        Some(8),
        "an accepted stream proves the server saw slot 8, not that it refused it"
    );
}

/// ⚠️ tonic builds its own `OUT_OF_RANGE` for an oversized message, which a replay can meet.
/// Mutation this is written against: `Ending::verdict` reading the code before `delivered`.
#[tokio::test]
async fn a_stream_failing_out_of_range_after_delivering_keeps_its_mark() {
    let server = test_geyser_server::start(vec![
        leaves_the_mark_at_8(),
        ScriptedSession::Stream(vec![
            routable(12),
            Action::Fail(Status::out_of_range("message too large")),
        ]),
        ScriptedSession::closes_empty(),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 1)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("one empty close after two churns exhausts a budget of one");

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(
        server.resume_points(),
        vec![None, Some(8), Some(10)],
        "a stream that delivered was accepted: its own mark stands, refusal code or not"
    );
}

/// ⚠️ An exhausted quota can look exactly like this, with no code to read.
/// Mutation this is written against: `Ending::verdict` classing `StreamClosed { delivered: false }`
/// as `Refused`.
#[tokio::test]
async fn a_clean_close_that_delivered_nothing_keeps_the_mark() {
    assert_eq!(mark_after(ScriptedSession::closes_empty()).await, Some(8));
}

/// The price of keeping the mark: a server refusing it under another code stops the indexer.
/// Mutation this is written against: `settle` handing `ServerFailed` to `churn`.
#[tokio::test]
async fn a_kept_mark_refused_for_ever_exhausts_the_budget() {
    let server = test_geyser_server::start(vec![
        leaves_the_mark_at_8(),
        refuses_subscribe(Status::unauthenticated("the key expired")),
        refuses_subscribe(Status::unauthenticated("the key expired")),
        refuses_subscribe(Status::unauthenticated("the key expired")),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 3)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("the budget must run out rather than redial a refusing server for ever");

    match outcome {
        Err(GrpcListenerError::RetriesExhausted {
            attempts: 3,
            last_error,
        }) => assert!(
            last_error.contains("the key expired"),
            "the budget must run out on the refusal, not on something else: {last_error}"
        ),
        other => panic!("three refusals must exhaust a budget of three: {other:?}"),
    }
    assert_eq!(
        server.resume_points(),
        vec![None, Some(8), Some(8), Some(8)],
        "every attempt asks for slot 8: the indexer stops rather than leave a gap"
    );
}

/// Not `#[tokio::test]`: it reads the counter. Mutation this is written against: removing the
/// increment (0), or counting a refusal with no mark held (2).
#[test]
fn a_mark_given_up_is_counted_once_by_its_code() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let server = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                let server = test_geyser_server::start(vec![
                    // No mark yet: nothing is given up.
                    fails_before_delivering(Status::out_of_range("no mark to refuse")),
                    leaves_the_mark_at_8(),
                    fails_before_delivering(Status::out_of_range("slot 8 is gone")),
                    ScriptedSession::closes_empty(),
                ])
                .await;
                let (downstream, _consumer) = mpsc::channel(4);

                let outcome = timeout(
                    TEST_DEADLINE,
                    listener_for(&server, 2)
                        .await
                        .run(downstream, CancellationToken::new()),
                )
                .await
                .expect("the budget must run out");
                assert!(outcome.is_err(), "{outcome:?}");
                server
            })
    });

    assert_eq!(server.resume_points(), vec![None, None, Some(8), None]);

    let abandoned: Vec<_> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_grpc_resume_marks_abandoned_total"
        })
        .map(|(key, _, _, value)| {
            let reason = key
                .key()
                .labels()
                .find(|l| l.key() == "reason")
                .map(|l| l.value().to_string());
            (reason, value)
        })
        .collect();
    assert_eq!(
        abandoned,
        vec![(Some("out_of_range".to_string()), DebugValue::Counter(1))],
        "one mark was given up, under the code that refused it"
    );
}

/// ⚠️ A refused dial produces nothing, like an empty close, but no one refused `from_slot`.
/// Mutation this owns: `Connector::open`'s `connect()` failure returning `Ending::Failed`; driven
/// at `attempt`, since `run` would need a port dead for one attempt only, a timing assertion.
#[tokio::test]
async fn a_failure_before_contact_keeps_the_mark_it_never_offered() {
    let listener = GrpcListener::new(
        Endpoint::for_tests(&a_closed_port().await, None),
        1,
        STALL_TIMEOUT,
    );
    listener.watch(PROTOCOL).await;

    // Built as `run` builds them, none of them failing: what is missing is a server.
    let credential =
        Credential::new(listener.connector.endpoint().header()).expect("no header is valid");
    let interceptor = CredentialInterceptor::new(&credential).expect("no header to reject");
    let channel = listener
        .connector
        .channel_endpoint()
        .expect("a plaintext loopback URL is a gRPC endpoint");
    // The attempt asks for slot 8, the mark a previous break left.
    let request = listener
        .subscribe_request(Some(8))
        .await
        .expect("one protocol is watched");
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener.connector.attempt(
            &channel,
            &interceptor,
            request,
            &downstream,
            &CancellationToken::new(),
        ),
    )
    .await
    .expect("a refused dial fails at once; it does not hang to the connect timeout");

    let error = match &outcome {
        Ending::Unreachable { error } => error,
        // Naming the impostor: reading this failure as a stream's ending is the defect itself.
        Ending::Failed { error, .. } => {
            panic!("the dial was refused, so nothing answered — yet: {error}")
        }
        _ => panic!("a refused dial ends the attempt before any stream exists"),
    };
    assert!(
        error.starts_with("connect:"),
        "the dial is the site that gave way, and an operator reads which one: {error}"
    );

    assert_eq!(
        outcome.verdict().next_resume_from(Some(8)),
        Some(8),
        "nothing was asked of anyone, so slot 8 was refused by no one: the \
         attempt after this one must ask for it again"
    );
}

/// ⚠️ tonic maps our own broken connection into a `Status` too, which looks like a refused replay.
/// Mutation this owns: `Connector::subscribe` losing its `reached_the_service` guard, so every
/// `Status` counts as the server's answer.
#[tokio::test]
async fn a_connection_cut_before_the_server_answered_keeps_the_mark() {
    let port = CuttingPort::bind().await;
    let listener = GrpcListener::new(Endpoint::for_tests(&port.url, None), 1, STALL_TIMEOUT);
    listener.watch(PROTOCOL).await;

    let credential =
        Credential::new(listener.connector.endpoint().header()).expect("no header is valid");
    let interceptor = CredentialInterceptor::new(&credential).expect("no header to reject");
    let channel = listener
        .connector
        .channel_endpoint()
        .expect("a plaintext loopback URL is a gRPC endpoint");
    let request = listener
        .subscribe_request(Some(8))
        .await
        .expect("one protocol is watched");
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener.connector.attempt(
            &channel,
            &interceptor,
            request,
            &downstream,
            &CancellationToken::new(),
        ),
    )
    .await
    .expect("a cut connection fails the RPC at once");

    let error = match &outcome {
        Ending::Unreachable { error } => error,
        Ending::Failed { error, .. } => {
            panic!("the connection was cut, so the server never answered — yet: {error}")
        }
        _ => panic!("a cut connection ends the attempt before any stream exists"),
    };
    // The site is the point: this fault arrives through `subscribe`.
    assert!(
        error.starts_with("subscribe:"),
        "the RPC is where a cut connection surfaces, and the log must say so: {error}"
    );

    assert_eq!(
        outcome.verdict().next_resume_from(Some(8)),
        Some(8),
        "a `Status` our own transport produced is nobody's verdict on slot 8: \
         the attempt after this one must ask for it again"
    );
}

/// ⚠️ Charged, not churn, or a dead endpoint would be redialled once a second for ever.
/// Mutation this is written against: `Ending::verdict` classing `Unreachable` as
/// `Delivered { mark: None }`; it only hangs, so the deadline is the assertion.
#[tokio::test]
async fn a_dial_that_never_connects_spends_the_budget() {
    let url = a_closed_port().await;
    let listener = Arc::new(GrpcListener::new(
        Endpoint::for_tests(&url, None),
        3,
        STALL_TIMEOUT,
    ));
    listener.watch(PROTOCOL).await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener.run(downstream, CancellationToken::new()),
    )
    .await
    .expect("a budget of three must run out rather than redial for ever");

    assert!(
        matches!(
            outcome,
            Err(GrpcListenerError::RetriesExhausted { attempts: 3, .. })
        ),
        "three dials, three charges, then a stop: {outcome:?}"
    );
}

/// ⚠️ An idle stream is where the process spends its life.
/// Mutation this is written against: removing the `shutdown.cancelled()` branch from
/// `Connector::read`'s `select!`; it only hangs, so the deadline is the assertion.
#[tokio::test]
async fn a_shutdown_reaches_a_listener_parked_on_a_silent_stream() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![Action::Hold])]).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();

    let listener = listener_for(&server, 1).await;
    let running = tokio::spawn(listener.run(downstream, shutdown.clone()));

    // Not a sleep: the stream must be open first, or this tests the loop's `is_cancelled` check.
    wait_for_subscriptions(&server, 1).await;
    shutdown.cancel();

    let outcome = timeout(TEST_DEADLINE, running)
        .await
        .expect("a silent stream must not swallow a stop request")
        .expect("no panic");

    assert!(
        outcome.is_ok(),
        "a requested shutdown is a clean stop, not a failure: {outcome:?}"
    );
}

/// ⚠️ Two transactions of one slot, released in one call: the second meets the full channel.
/// Mutation this is written against: `session/delivery.rs`'s back-pressure wait made unconditional,
/// `self.downstream.send(ingested).await` with no `select!`.
#[tokio::test]
async fn a_shutdown_reaches_a_session_parked_on_a_full_consumer() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![
        routable(10),
        routable(10),
        Action::send(slot_finalized(10, None)),
        Action::send(block_meta(10, Some(BLOCK_TIME))),
        Action::Hold,
    ])])
    .await;
    // Room for exactly one: the second payload of slot 10 parks.
    let (downstream, consumer) = mpsc::channel(1);
    let shutdown = CancellationToken::new();

    let listener = listener_for(&server, 1).await;
    let running = tokio::spawn(listener.run(downstream, shutdown.clone()));

    let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
    while consumer.is_empty() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "nothing reached the consumer, so nothing is parked behind it"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    shutdown.cancel();

    timeout(TEST_DEADLINE, running)
        .await
        .expect("the back-pressure wait must not swallow a stop request")
        .expect("no panic")
        .expect("a requested shutdown is a clean stop");

    assert_eq!(
        server.requests().len(),
        1,
        "a stop is a stop: no reconnection after it"
    );
}

/// A consumer that is gone ends the listener cleanly, without reconnecting.
/// Mutation this is written against: treating `DownstreamClosed` as churn; the subscription count
/// names it.
#[tokio::test]
async fn a_vanished_consumer_stops_the_listener_without_retrying() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![
        routable(10),
        Action::send(slot_finalized(10, None)),
        Action::send(block_meta(10, Some(BLOCK_TIME))),
        Action::Hold,
    ])])
    .await;
    let (downstream, consumer) = mpsc::channel(4);
    drop(consumer);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_for(&server, 1)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("a vanished consumer must stop the listener, not spin it");

    assert!(
        outcome.is_ok(),
        "nothing left to feed is a clean stop, not a failure: {outcome:?}"
    );
    assert_eq!(
        server.requests().len(),
        1,
        "and it is not a reconnection: retrying would not bring the consumer back"
    );
}

// ── a stream that stalls without closing ────────────────────────────

/// Short enough to wait through, long enough that block-metas 50 ms apart never near it.
const TEST_STALL: Duration = Duration::from_millis(300);

/// Room for a second stall, should the stop arrive late: the assertions read what was asked for.
const STALL_TEST_ATTEMPTS: u32 = 10;

/// Run a listener against `script` until the server has been subscribed to
/// `subscriptions` times, then stop it and hand back the server.
async fn run_until_subscribed(
    script: Vec<ScriptedSession>,
    subscriptions: usize,
) -> ScriptedGeyserHandle {
    let server = test_geyser_server::start(script).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();

    let listener = listener_with_stall(&server, STALL_TEST_ATTEMPTS, TEST_STALL).await;
    let running = tokio::spawn(listener.run(downstream, shutdown.clone()));

    wait_for_subscriptions(&server, subscriptions).await;
    shutdown.cancel();

    let outcome = timeout(TEST_DEADLINE, running)
        .await
        .expect("a requested stop must end the run")
        .expect("no panic");
    assert!(
        outcome.is_ok(),
        "a requested stop is a clean one: {outcome:?}"
    );

    server
}

/// ⚠️ A server can stop delivering and keep the connection (`rpcpool/yellowstone-grpc` #25, #175).
/// Mutation this is written against: removing the stall arm of `Connector::read`'s `select!`, or
/// `stalled` handing back `resume_from: None`. Not `#[tokio::test]`: it reads the counter.
#[test]
fn a_stalled_stream_is_resubscribed_at_its_mark() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let server = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(run_until_subscribed(
                vec![
                    ScriptedSession::Stream(vec![
                        Action::send(block_meta(10, Some(BLOCK_TIME))),
                        Action::Hold,
                    ]),
                    ScriptedSession::Stream(vec![Action::Hold]),
                ],
                2,
            ))
    });

    assert_eq!(
        server.resume_points(),
        vec![None, Some(8)],
        "the stalled session closed slot 10, so the next one asks for it again, \
         rewound — a stall is churn, and keeps the mark"
    );

    // At least one: the second session holds too, and a late stop on a loaded runner stalls it as well.
    let stalls = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_grpc_stalls_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "site" && l.value() == "stream")
        })
        .map(|(_, _, _, value)| value);
    assert!(
        matches!(stalls, Some(DebugValue::Counter(n)) if n >= 1),
        "a stall is the one ending nothing else reports — it must be counted, \
         under the stream site: {stalls:?}"
    );
}

/// ⚠️ Against the peaks exported nowhere, or at registration, which runs whichever source: a
/// quiet gRPC buffer would show no series, and a WebSocket indexer would show gRPC ones.
#[test]
fn a_run_exports_the_buffer_peaks() {
    use metrics_util::debugging::DebuggingRecorder;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(run_until_subscribed(
                vec![ScriptedSession::Stream(vec![Action::Hold])],
                1,
            ))
    });

    let snapshot = snapshotter.snapshot().into_vec();
    for name in [
        "yog_indexer_grpc_pending_payloads_peak",
        "yog_indexer_grpc_pending_slot_lag_peak",
    ] {
        assert!(
            snapshot
                .iter()
                .any(|(key, _, _, _)| key.key().name() == name),
            "{name} must exist before the stream raises it"
        );
    }
}

/// ⚠️ A server pings from a task of its own, so a stopped data path keeps pinging.
/// Mutation this is written against: resetting the clock on every message rather than on a block-meta.
#[tokio::test]
async fn pings_do_not_keep_a_stalled_stream_alive() {
    let server = run_until_subscribed(
        vec![
            ScriptedSession::Stream(vec![
                Action::send(block_meta(10, Some(BLOCK_TIME))),
                Action::PingEvery(TEST_STALL / 6),
            ]),
            ScriptedSession::Stream(vec![Action::Hold]),
        ],
        2,
    )
    .await;

    assert_eq!(server.resume_points(), vec![None, Some(8)]);
}

/// A slow but live stream is not restarted.
/// Mutation this is written against: never resetting the clock, so it measures the session's age.
#[tokio::test]
async fn a_slow_but_live_stream_is_not_restarted() {
    let mut actions = Vec::new();
    for slot in 10..30 {
        actions.push(Action::send(block_meta(slot, Some(BLOCK_TIME))));
        actions.push(Action::Wait(TEST_STALL / 6));
    }
    actions.push(Action::Fail(Status::unavailable("the scripted break")));

    let server = run_until_subscribed(
        vec![
            ScriptedSession::Stream(actions),
            ScriptedSession::Stream(vec![Action::Hold]),
        ],
        2,
    )
    .await;

    assert_eq!(
        server.resume_points(),
        vec![None, Some(27)],
        "the first session must have run to its scripted break, slot 29 — an \
         earlier mark means it was cut as stalled while block-metas were arriving"
    );
}

/// A stream that stalls having delivered nothing spends the retry budget.
/// Mutation this is written against: `stalled` reporting `delivered: true`, which restarts the
/// budget each time.
#[tokio::test]
async fn a_stall_that_delivered_nothing_spends_the_budget() {
    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(vec![Action::Hold]),
        ScriptedSession::Stream(vec![Action::Hold]),
    ])
    .await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener_with_stall(&server, 2, TEST_STALL)
            .await
            .run(downstream, CancellationToken::new()),
    )
    .await
    .expect("a budget of two must run out rather than wait for ever");

    match outcome {
        Err(GrpcListenerError::RetriesExhausted {
            attempts: 2,
            last_error,
        }) => assert!(
            last_error.contains("stalled"),
            "the budget must run out on the stall, not on something else: {last_error}"
        ),
        other => panic!("two silent sessions must exhaust a budget of two: {other:?}"),
    }
    assert_eq!(server.requests().len(), 2);
}

/// ⚠️ A stall is no refusal: the decision to stop waiting is ours, so the mark is kept.
/// Mutation this is written against: `Ending::verdict` classing a stall that delivered nothing as
/// `Refused`.
#[tokio::test]
async fn a_stall_that_delivered_nothing_keeps_the_mark() {
    let server = run_until_subscribed(
        vec![
            ScriptedSession::Stream(vec![
                Action::send(block_meta(10, Some(BLOCK_TIME))),
                Action::Hold,
            ]),
            ScriptedSession::Stream(vec![Action::Hold]),
            ScriptedSession::Stream(vec![Action::Hold]),
        ],
        3,
    )
    .await;

    assert_eq!(
        server.resume_points(),
        vec![None, Some(8), Some(8)],
        "the silent second attempt is no verdict on slot 10 — the third asks again"
    );
}

/// ⚠️ A front end with a wedged backend never sends headers, and the stream clock never starts.
/// Mutation this is written against: `subscribe` awaited without the timeout, or the timeout
/// giving the mark up.
#[tokio::test]
async fn a_subscribe_that_is_never_answered_is_bounded_and_keeps_the_mark() {
    let server = run_until_subscribed(
        vec![
            ScriptedSession::Stream(vec![
                Action::send(block_meta(10, Some(BLOCK_TIME))),
                Action::Fail(Status::unavailable("the scripted break")),
            ]),
            ScriptedSession::NeverAnswer,
            ScriptedSession::Stream(vec![Action::Hold]),
        ],
        3,
    )
    .await;

    assert_eq!(server.resume_points(), vec![None, Some(8), Some(8)]);
}

/// Mutation this is written against: the subscribe site not counting its stall.
#[test]
fn a_subscribe_that_is_never_answered_is_counted_under_its_site() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(run_until_subscribed(
                vec![
                    ScriptedSession::NeverAnswer,
                    ScriptedSession::Stream(vec![Action::Hold]),
                ],
                2,
            ))
    });

    let stalls = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_grpc_stalls_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "site" && l.value() == "subscribe")
        })
        .map(|(_, _, _, value)| value);
    assert!(
        matches!(stalls, Some(DebugValue::Counter(n)) if n >= 1),
        "a subscribe never answered is counted under its own site: {stalls:?}"
    );
}

/// ⚠️ The stall timeout outlasts a container's stop grace, so `docker compose stop` would kill it.
/// Mutation this is written against: awaiting the bounded `subscribe` without the `select!` on the token.
#[tokio::test]
async fn a_shutdown_reaches_a_subscribe_that_is_never_answered() {
    let server = test_geyser_server::start(vec![ScriptedSession::NeverAnswer]).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();

    let listener = listener_with_stall(&server, 1, Duration::from_secs(3600)).await;
    let running = tokio::spawn(listener.run(downstream, shutdown.clone()));

    wait_for_subscriptions(&server, 1).await;
    shutdown.cancel();

    let outcome = timeout(TEST_DEADLINE, running)
        .await
        .expect("a stop request must not wait out the stall timeout")
        .expect("no panic");
    assert!(
        outcome.is_ok(),
        "a requested stop is a clean one: {outcome:?}"
    );
}
