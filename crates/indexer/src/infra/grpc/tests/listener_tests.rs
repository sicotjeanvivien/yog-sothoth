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
        .subscription()
        .await
        .map(|(request, _)| request)
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

use tokio::{net::TcpListener, time::timeout};
use tonic::Status;

use crate::infra::grpc::{
    test_fixtures::{
        PROTOCOL, RECORDER_PANIC, WrittenSlots, block_meta, ping, received_slots_unwritten,
        slot_finalized, transaction,
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
/// ⚠️ A name that is no protocol drops it before the buffer, and a slot it holds open settles.
fn routable(slot: u64) -> Action {
    Action::send(transaction(slot, &[PROTOCOL.as_str()]))
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

/// ⚠️ A replay can fall behind the chain for good: a cut, an error or a clean close, mid-block,
/// resubscribes live. Mutation this is written against: an attempt asking for a `from_slot`.
#[tokio::test]
async fn a_cut_mid_block_resubscribes_live() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction, then the stream breaks with the slot still open.
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the connection was reset")),
        ]),
        // Slot 20's transaction, then a clean end of stream with the slot still open.
        ScriptedSession::Stream(vec![routable(20)]),
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

    assert!(
        matches!(
            outcome,
            Err(GrpcListenerError::RetriesExhausted { attempts: 1, .. })
        ),
        "the two cuts are churn, the empty close is the one charge: {outcome:?}"
    );
    assert_eq!(
        server.requested_from_slots(),
        vec![None, None, None],
        "slots 10 and 20 were cut open, and no attempt asks for them again"
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

fn refuses_subscribe(status: Status) -> ScriptedSession {
    ScriptedSession::Refuse(status)
}

/// A server refusing every subscription stops the indexer rather than be redialled for ever.
/// Mutation this is written against: `settle` handing `ServerFailed` to `churn`.
#[tokio::test]
async fn a_server_refusing_for_ever_exhausts_the_budget() {
    let server = test_geyser_server::start(vec![
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
    assert_eq!(server.requests().len(), 3);
}

/// ⚠️ A refused dial produces nothing, like an empty close, but nobody answered it.
/// Mutation this owns: `Connector::open`'s `connect()` failure returning `Ending::Failed`; driven
/// at `attempt`, since `run` would need a port dead for one attempt only, a timing assertion.
#[tokio::test]
async fn a_failure_before_contact_is_unreachable_at_the_dial() {
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
    let request = listener
        .subscription()
        .await
        .map(|(request, _)| request)
        .expect("one protocol is watched");
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener.connector.attempt(
            &channel,
            &interceptor,
            request,
            &downstream,
            received_slots_unwritten(),
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
}

/// ⚠️ tonic maps our own broken connection into a `Status` too, which looks like the server's.
/// Mutation this owns: `Connector::subscribe` losing its `reached_the_service` guard, so every
/// `Status` counts as the server's answer.
#[tokio::test]
async fn a_connection_cut_before_the_server_answered_is_unreachable() {
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
        .subscription()
        .await
        .map(|(request, _)| request)
        .expect("one protocol is watched");
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener.connector.attempt(
            &channel,
            &interceptor,
            request,
            &downstream,
            received_slots_unwritten(),
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
}

/// ⚠️ Charged, not churn, or a dead endpoint would be redialled once a second for ever.
/// Mutation this is written against: `Ending::verdict` classing `Unreachable` as
/// `Delivered { error: None }`; it only hangs, so the deadline is the assertion.
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
/// Mutation this is written against: removing the stall arm of `Connector::read`'s `select!`.
/// Not `#[tokio::test]`: it reads the counter.
#[test]
fn a_stalled_stream_is_resubscribed_live() {
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
        server.requested_from_slots(),
        vec![None, None],
        "a stall is churn, and the next session starts live"
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

    assert_eq!(server.requested_from_slots(), vec![None, None]);
}

/// A slow but live stream is not restarted; how far its span reaches says where it was cut.
/// Mutation this is written against: never resetting the clock, so it measures the session's age.
#[tokio::test]
async fn a_slow_but_live_stream_is_not_restarted() {
    let mut actions = Vec::new();
    for slot in 10..30 {
        actions.push(timed_meta(slot));
        actions.push(Action::Wait(TEST_STALL / 6));
    }
    actions.push(Action::Fail(Status::unavailable("the scripted break")));

    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(actions),
        ScriptedSession::Stream(vec![Action::Hold]),
    ])
    .await;
    let recorded = Arc::new(WrittenSlots::default());
    let listener = GrpcListener::new(
        Endpoint::for_tests(server.url(), None),
        STALL_TEST_ATTEMPTS,
        TEST_STALL,
    )
    .with_slots_recorder(SlotsRecorder::new(recorded.clone(), RECORD_TICK));
    listener.watch(PROTOCOL).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();
    let run = tokio::spawn(Arc::new(listener).run(downstream, shutdown.clone()));

    // Slot 29, the scripted break, settles up to 26: a span ending earlier was cut as stalled.
    wait_for_sessions(&recorded, &[(12, 26)]).await;

    shutdown.cancel();
    run.await.expect("the listener ends").expect("on shutdown");
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

/// ⚠️ A front end with a wedged backend never sends headers, and the stream clock never starts.
/// Mutation this is written against: `subscribe` awaited without the timeout.
#[tokio::test]
async fn a_subscribe_that_is_never_answered_is_bounded() {
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

    assert_eq!(server.requested_from_slots(), vec![None, None, None]);
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

// ── what the sessions received for good ──────────────────────────────────────

/// A recorder period the tests can wait through.
const RECORD_TICK: Duration = Duration::from_millis(20);

/// A listener watching DAMM v2 whole, writing what it receives in memory.
async fn recorded_listener(
    server: &ScriptedGeyserHandle,
    max_attempts: u32,
) -> (Arc<GrpcListener>, Arc<WrittenSlots>) {
    let recorded = Arc::new(WrittenSlots::default());
    let listener = GrpcListener::new(
        Endpoint::for_tests(server.url(), None),
        max_attempts,
        STALL_TIMEOUT,
    )
    .with_slots_recorder(SlotsRecorder::new(recorded.clone(), RECORD_TICK));
    listener.watch(PROTOCOL).await;
    (Arc::new(listener), recorded)
}

fn timed_meta(slot: u64) -> Action {
    Action::send(block_meta(slot, Some(BLOCK_TIME + slot as i64)))
}

/// The widest span written per session, in the order the sessions started.
fn sessions(recorded: &WrittenSlots) -> Vec<(u64, u64)> {
    let mut widest: Vec<(chrono::DateTime<chrono::Utc>, u64, u64)> = Vec::new();
    for row in recorded.rows() {
        match widest
            .iter_mut()
            .find(|(key, _, _)| *key == row.session_started_at)
        {
            Some((_, from, to)) => {
                *from = (*from).min(row.first_slot);
                *to = (*to).max(row.last_slot);
            }
            None => widest.push((row.session_started_at, row.first_slot, row.last_slot)),
        }
    }
    widest.sort();
    widest.into_iter().map(|(_, from, to)| (from, to)).collect()
}

/// Wait until the sessions written are `expected`, or the deadline.
async fn wait_for_sessions(recorded: &WrittenSlots, expected: &[(u64, u64)]) {
    let waited = timeout(TEST_DEADLINE, async {
        while sessions(recorded) != expected {
            tokio::time::sleep(RECORD_TICK).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "recorded {:?}, expected {expected:?}",
        sessions(recorded)
    );
}

/// Mutation: the session no longer counting what it receives, or its last slot no longer widened.
#[tokio::test]
async fn a_session_writes_the_slots_it_received_skipped_ones_included() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![
        timed_meta(10),
        timed_meta(11),
        timed_meta(12),
        timed_meta(13),
        timed_meta(15),
        timed_meta(16),
        timed_meta(17),
        timed_meta(18),
        Action::Hold,
    ])])
    .await;
    let (listener, recorded) = recorded_listener(&server, 1).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();
    let run = tokio::spawn(listener.run(downstream, shutdown.clone()));

    // 10 and 11 open the session; settled stops two under the highest, at 16; 14 was skipped.
    wait_for_sessions(&recorded, &[(12, 15)]).await;

    shutdown.cancel();
    run.await.expect("the listener ends").expect("on shutdown");
}

/// ⚠️ Nothing replays a cut: the slots it can have cut short, on both sides, must be in no range.
/// Mutation this is written against: `settled_below` answering from the highest block-meta, or a
/// session counting its first slots.
#[tokio::test]
async fn a_cut_leaves_the_slots_it_cut_short_in_no_range() {
    let mut first = vec![routable(15)];
    first.extend((10..=20).map(timed_meta));
    first.push(Action::Fail(Status::unavailable("the provider restarted")));
    let server = test_geyser_server::start(vec![
        // Slot 15's transaction waits for its status when the stream breaks.
        ScriptedSession::Stream(first),
        ScriptedSession::Stream((30..=36).map(timed_meta).chain([Action::Hold]).collect()),
    ])
    .await;
    let (listener, recorded) = recorded_listener(&server, 2).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();
    let run = tokio::spawn(listener.run(downstream, shutdown.clone()));

    // Each session counts from its first slot plus two; slot 15 holds the first one under 13,
    // and the second one, live, settles below 34.
    wait_for_sessions(&recorded, &[(12, 12), (32, 33)]).await;

    shutdown.cancel();
    run.await.expect("the listener ends").expect("on shutdown");
}

/// Mutation: one session key for the whole listener, so a reconnection extends the span before.
#[tokio::test]
async fn each_connection_writes_its_own_span() {
    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(
            (10..=15)
                .map(timed_meta)
                .chain([Action::Fail(Status::unavailable("the provider restarted"))])
                .collect(),
        ),
        ScriptedSession::Stream((20..=25).map(timed_meta).chain([Action::Hold]).collect()),
    ])
    .await;
    let (listener, recorded) = recorded_listener(&server, 2).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();
    let run = tokio::spawn(listener.run(downstream, shutdown.clone()));

    wait_for_sessions(&recorded, &[(12, 12), (22, 22)]).await;

    shutdown.cancel();
    run.await.expect("the listener ends").expect("on shutdown");
}

/// Mutation: the recorder spawned in a task of its own, its result thrown away.
#[tokio::test]
async fn a_recorder_panic_is_the_listener_panic() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![
        timed_meta(10),
        timed_meta(11),
        timed_meta(13),
        timed_meta(14),
        timed_meta(15),
        timed_meta(16),
        Action::Hold,
    ])])
    .await;
    let (listener, recorded) = recorded_listener(&server, 1).await;
    recorded.panic_on_write();
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();

    let ended = timeout(
        TEST_DEADLINE,
        tokio::spawn(listener.run(downstream, shutdown)),
    )
    .await
    .expect("the panic ends the listener while its stream holds");

    let panic = ended.expect_err("the listener panics").into_panic();
    let message = panic
        .downcast_ref::<String>()
        .expect("the recorder's formatted panic message");
    assert!(message.contains(RECORDER_PANIC), "{message}");
}

/// Mutation: the recorder no longer stopped when the attempts end, so a give-up waits on it for ever.
#[tokio::test]
async fn a_listener_that_gives_up_stops_its_recorder() {
    let server =
        test_geyser_server::start(vec![ScriptedSession::Stream(vec![Action::send(ping())])]).await;
    let (listener, _recorded) = recorded_listener(&server, 1).await;
    let (downstream, _consumer) = mpsc::channel(4);

    let outcome = timeout(
        TEST_DEADLINE,
        listener.run(downstream, CancellationToken::new()),
    )
    .await
    .expect("the give-up ends the listener, its recorder with it");

    assert!(
        matches!(
            outcome,
            Err(GrpcListenerError::RetriesExhausted { attempts: 1, .. })
        ),
        "{outcome:?}"
    );
}

/// Mutation: the program-id filter removed, so a pool watched alone watches its protocol.
#[tokio::test]
async fn only_a_protocol_watched_whole_counts() {
    let pool = (PROTOCOL, Pubkey::new_from_array([7; 32]));
    let mut watched = HashSet::from([pool]);
    assert_eq!(&*whole_protocols(&watched), []);

    watched.insert((PROTOCOL, PROTOCOL.program_id()));
    assert_eq!(&*whole_protocols(&watched), [PROTOCOL]);
}
