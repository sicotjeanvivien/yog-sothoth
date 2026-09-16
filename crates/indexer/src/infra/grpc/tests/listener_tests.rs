//! Two kinds of test, and the second one used not to exist.
//!
//! **Before the loop** — `channel_endpoint` and `watch` are pure functions of
//! the configured endpoint and the watch set, and what they refuse is the
//! misconfiguration this path makes possible. What each refusal *says* is
//! `scheme_tests`'s; what is tested here is that this path builds its endpoint,
//! and asks.
//!
//! **Inside the loop** — the retry rule itself, driven against `test_geyser_server`.
//! A scripted server is what makes it reachable: every decision `run` makes is
//! a decision about *how a stream ended*, and nothing short of a server
//! produces those endings. See the header of that module for what this proves
//! and what it does not; the short version is that it validates this client
//! against our model of the server, and that
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` is still the ticket where
//! the model meets a real one.

use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> GrpcListener {
    GrpcListener::new(Endpoint::for_tests(url, None), 1)
}

/// Accepted means built: TLS and keep-alive are configured on the way out, so
/// this also says a plaintext `http://` endpoint survives `tls_config`.
#[test]
fn an_http_endpoint_is_accepted() {
    for url in [
        "https://grpc.example.com:443",
        "http://127.0.0.1:10000",
        "HTTPS://x.io",
    ] {
        assert!(
            listener(url).channel_endpoint().is_ok(),
            "{url} is a gRPC endpoint"
        );
    }
}

/// ⚠️ The defect this call was written for, and it was found by *reading a
/// successful-looking run*: the gRPC path was launched against the `wss://` URL
/// of `.env`, produced ten retries with backoff, and that was taken for "a
/// transport error, as expected". Deleting the scheme check from
/// `channel_endpoint` turns this red; `scheme_tests` would stay green.
#[test]
fn a_websocket_endpoint_is_refused_before_the_loop() {
    let detail = listener("wss://api.example.com")
        .channel_endpoint()
        .expect_err("a WebSocket URL is not a gRPC endpoint")
        .to_string();

    // The scheme check's own wording, and not merely `InvalidEndpoint`:
    // `from_shared` and `tls_config` raise that variant too, so a tonic that
    // one day refused `wss://` itself would keep a variant-only assertion green
    // while operators lost the message that says what to write.
    assert!(detail.contains("`wss` scheme"), "{detail}");
}

/// ⚠️ **A URL with no scheme gets past `from_shared`** — `grpc.example.com:443`
/// is a valid URI — so it is the scheme check, and only it, that has to name the
/// mistake. `scheme_tests` pins the wording; this pins that the URL gets there.
#[test]
fn a_url_without_a_scheme_reaches_the_scheme_check() {
    let detail = listener("grpc.example.com:443")
        .channel_endpoint()
        .expect_err("a schemeless URL is not a gRPC endpoint")
        .to_string();

    assert!(detail.contains("no scheme"), "{detail}");
}

/// ⚠️ **A watched protocol is its program id, and this is the only place that
/// says so** on this path. The listener holds one set of addresses and no
/// scope, so the translation lives in `watch` — and a `watch` that inserted
/// anything else would open a stream that matches nothing. A pool is taken as
/// given, grouped into the same protocol's filter.
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
// Everything below drives `GrpcListener::run` itself, against `test_geyser_server`.
// Until 16 September 2026 nothing did: the six arms of `run`'s `match` are the
// rule that decides what restarts the retry budget, what charges it, and where
// the next attempt resumes from, and every one of the five defects that rule
// has had was found by reading it. None could have been found by running it.
//
// ⚠️ **Every rule below has an owner**: one test whose failure message names
// that rule, listed on the test as the mutation it is written against. A
// mutation that reddens a test which does *not* own the rule sends the reader
// to the wrong file, so where a test needs a rule it does not own — the budget
// reset, to reach a second attempt at all — it asserts a *prefix* of what it
// observed rather than the whole of it.
//
// ⚠️ **One entanglement cannot be removed, and pretending otherwise is how this
// file already went wrong once.** The three tests that prove a resume point is
// *given up*, or *kept across an attempt that had none*, need one to exist
// first, and the only things that create one are the two endings that prove a
// resume point is *kept*. So breaking the mark an errored stream carries
// reddens four tests, not one:
// `an_error_mid_block_resumes_from_the_slot_that_was_cut`, which owns it and
// names it, plus the three that borrowed it as scaffolding. The owner is what
// makes that readable; it was missing until a review of this change found the
// arm had none.
//
// ⚠️ **Where the resume mutations live moved on 16 September 2026**, with the
// fix for a delivered session that had no mark of its own. `run`'s arms no
// longer write `resume_from`: the rule is `Attempt::next_resume_from`, one
// expression with one branch per ending, and the *per-ending* facts it reads are
// built in `connect_and_stream`. So a mutation that used to belong to one arm
// now belongs to one of those two places, and each annotation below says which.

use tokio::time::timeout;
use tonic::Status;

use crate::infra::grpc::{
    test_fixtures::{PROTOCOL, block_meta, ping, transaction, unroutable_transaction},
    test_geyser_server::{self, Action, ScriptedGeyserHandle, ScriptedSession},
};

/// Nothing here should take seconds; `run`'s own backoff starts at one and the
/// longest script waits through three of them. What this bounds is the failure
/// mode every mutation below produces — a listener that retries for ever, or a
/// wait that never wakes. Without it the suite would **hang instead of going
/// red**, which is a guard that does not guard.
const TEST_DEADLINE: Duration = Duration::from_secs(30);

/// A block time, and any one will do: no assertion below reads it. It is here
/// because a block-meta without one gives its slot up rather than releasing it,
/// which would silently defeat the two consumer tests.
const BLOCK_TIME: i64 = 1_700_000_000;

/// A listener pointed at a scripted server, watching the one protocol with a
/// working extractor.
///
/// ⚠️ The `watch` is not decoration: `build_request` refuses an empty
/// subscription with `NoSubscriptionTargets` **before** the retry loop, so a
/// test that forgot it would never reach a single one of these rules.
async fn listener_for(server: &ScriptedGeyserHandle, max_attempts: u32) -> Arc<GrpcListener> {
    let listener = Arc::new(GrpcListener::new(
        Endpoint::for_tests(server.url(), None),
        max_attempts,
    ));
    listener.watch(Protocol::MeteoraDammV2).await;
    listener
}

/// A transaction of `slot`, carrying the filter name that makes it routable.
///
/// ⚠️ Any other name and `on_transaction` drops it before the buffer, so
/// `resume_from` stays `None` and the resumption tests assert nothing. Which is
/// why the name comes from `test_fixtures::PROTOCOL` and is not spelled again here:
/// that constant *is* the rule, and a second copy of a rule is how this module
/// has produced defects before.
fn routable(slot: u64) -> Action {
    Action::send(transaction(slot, &[PROTOCOL.as_str()]))
}

/// A transaction of `slot` the pipeline cannot route.
///
/// ⚠️ The pair it produces — delivered, with nothing to resume from — is stated
/// once, on `test_fixtures::unroutable_transaction`, and pinned on its own by
/// `session_tests`. What is built here is only the `Action` that puts it on a
/// stream, exactly as `routable` does: the rule is not spelled twice, for the
/// reason the constant next to `PROTOCOL` gives.
fn unroutable(slot: u64) -> Action {
    Action::send(unroutable_transaction(slot))
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

/// ⚠️ **A stream that pings and closes having delivered nothing is a failing
/// attempt, not churn** — and a Yellowstone server pings shortly after
/// `subscribe`, so this is the ordinary shape of a refusal: an exhausted quota,
/// a token refused at stream level, a `from_slot` past retention. Reading the
/// ping as delivery puts it in the churn arm, where the budget restarts and the
/// provider is redialled once a second for ever, `max_attempts` never reached.
///
/// Mutation this is written against: `session.rs`, the `Ping` arm setting
/// `received_data = true`. It does not turn the error into a different one —
/// the budget still runs out eventually, on the exhausted script — so what says
/// it is the **number of attempts**, which is the quantity the rule is about.
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

/// ⚠️ **A session that delivered and then errored is churn**, and this arm
/// forgot it until 10 September 2026. `Err(Status)` is how a long-lived stream
/// ordinarily breaks — a GOAWAY, an h2 reset, a nightly provider restart — so
/// with the budget charged and never restarted, `attempt` climbed across
/// sessions and the tenth restart shut the indexer down having lost nothing.
///
/// The arithmetic is the assertion: *delivers then errors*, *closes empty*,
/// *closes empty*, with a budget of two, is **three** subscriptions. Without
/// the reset the first one is charged and it is two.
///
/// Mutation this is written against: removing `attempt = 0` from the
/// `Failed { delivered: true }` arm.
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

/// ⚠️ **And the same resume point, when the cut is an error rather than a clean
/// close.** Which of the two a provider sends is not ours to choose — a TCP
/// reset and a graceful GOAWAY cut the same block in the same place — so the
/// arm that handles the error must carry the mark exactly as its twin does.
///
/// Found by review of this change, 16 September 2026: the mark an errored
/// stream carries had **no owner**. Two tests used it as scaffolding to produce
/// their `Some(8)`, so dropping it turned both of them red with messages about
/// other rules — the very pattern the section header above forbids, left
/// unapplied on one arm by the commit that wrote the rule.
///
/// Mutation this is written against: `connect_and_stream`'s `Err(status)`
/// ending handing back `resume_from: None` instead of `session.resume_from()`.
/// That is where this ending's mark is now built — `next_resume_from` no longer
/// tells the two endings apart, which is the point of it.
#[tokio::test]
async fn an_error_mid_block_resumes_from_the_slot_that_was_cut() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction, then the stream breaks — no block-meta, so the
        // slot is still open when the session dies.
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

/// ⚠️ **And a stream that delivered and then closed *cleanly* is churn too** —
/// the same rule, on the arm next door. A graceful GOAWAY, a provider draining
/// a node before a restart and an idle-timeout close all end as `Ok(None)`
/// rather than `Err(Status)`, and which of the two a given provider sends is
/// not ours to choose. Charging the budget for them makes `attempt` climb
/// across sessions until the *n*-th restart stops an indexer that has lost
/// nothing — the defect the `Failed` arm had until 10 September 2026, waiting
/// on its twin.
///
/// Found by review of this very change, 16 September 2026: the first version of
/// these tests claimed to cover every arm and left this one reset unobserved.
///
/// Mutation this is written against: removing `attempt = 0` from the
/// `StreamClosed { delivered: true }` arm.
#[tokio::test]
async fn a_stream_that_delivered_before_closing_cleanly_restarts_the_budget() {
    let server = test_geyser_server::start(vec![
        // No `Fail`: the actions simply run out, which the client sees as a
        // clean end of stream.
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

/// ⚠️ **The resume point is the oldest slot the session did not finish, and a
/// break mid-block is the case it exists for.** A transaction names a slot
/// still in flight: its payload sits in the buffer, and the buffer dies with
/// the session. So slot 10 arriving without its block-meta must be asked for
/// again — rewound by `REWIND_SLOTS`, because a block-meta does not promise its
/// slot's transactions have all arrived either.
///
/// Mutation this is written against: `connect_and_stream`'s `Ok(None)` ending
/// handing back `resume_from: None` instead of `session.resume_from()` — the
/// half `session_tests` cannot see, since it proves `resume_from` computes 8,
/// not that anything asks for it.
#[tokio::test]
async fn a_break_mid_block_resumes_from_the_slot_that_was_cut() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction, and then the stream ends — no block-meta, so
        // the slot is still open when the session dies.
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

/// ⚠️ **A session that delivered but has no mark of its own must not erase the
/// mark we already hold.** An absent mark is not a mark at zero: a transaction
/// matching no protocol filter is counted `Unroutable` and dropped before the
/// buffer, so the session ends `delivered` with `resume_from() == None` — and
/// reading that as "there is no resume point" threw away a still-valid one and
/// sent the attempt after it to the live edge. The transactions of the original
/// break are then never asked for again, and no event table can know a row is
/// missing.
///
/// The three attempts are the defect: *break mid-block* leaves slot 8, *one
/// unroutable transaction then a clean close* has nothing to offer, and the
/// third must still ask for 8.
///
/// Mutation this is written against: `Attempt::next_resume_from`'s delivered
/// branch returning the session's mark alone (`*resume_from`) instead of
/// `(*resume_from).or(held)`. It reddens nothing else — the two resumption
/// tests above hold `None` at that point, where `or` is invisible.
///
/// ⚠️ **The mark of the first attempt is scaffolding**, produced by the
/// `Failed { delivered: true }` ending as its siblings' is, and for the reason
/// the header gives: only the delivered arms can create a mark to observe.
#[tokio::test]
async fn an_attempt_with_nothing_to_resume_from_keeps_the_mark_we_hold() {
    let server = test_geyser_server::start(vec![
        // Slot 10's transaction and then a break — the mark is 10 rewound by two.
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the connection was reset")),
        ]),
        // Delivery the pipeline cannot route, then a clean close: this session
        // has nothing to resume from, and nothing is not "slot zero".
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
    // ⚠️ The whole vector, not a prefix, and it is safe here where its
    // neighbours' is not: removing either churn arm's `attempt = 0` still
    // leaves three subscriptions asking for these same three points, so no
    // budget rule can redden this test.
    assert_eq!(
        server.resume_points(),
        vec![None, Some(8), Some(8)],
        "the unroutable transaction gave that session nothing to resume from; \
         it must not take slot 8 away from the attempt after it"
    );
}

/// ⚠️ **A replay refused at the handshake is not asked for twice.** The slot
/// may be past the server's retention, and providers do not agree on how far
/// back that goes, so the next attempt starts from the live edge: the gap is
/// lost rather than looped on. No error text is read to decide it — only
/// whether the stream produced anything.
///
/// Mutation this is written against: `Attempt::next_resume_from`'s undelivered
/// branch returning `held` rather than `None`, which keeps the refused mark and
/// asks for it again. It is the mutation
/// `a_clean_close_that_delivered_nothing_gives_up_the_replay_point` also owns —
/// one branch now answers for both endings, and a branch cannot be corrected on
/// one ending and forgotten on the other, which is what four assignments
/// allowed. What separates the two tests is the ending each drives it with.
///
/// ⚠️ A **prefix** of the resume points, not all of them: reaching a second
/// attempt at all needs the budget reset of
/// `a_stream_that_delivered_before_breaking_restarts_the_budget`, and breaking
/// *that* rule changes how many attempts happen after the third. Asserting the
/// whole vector would make this test red for a defect it does not own.
#[tokio::test]
async fn a_refused_resume_point_is_not_asked_for_twice() {
    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the provider restarted")),
        ]),
        ScriptedSession::Refuse(Status::invalid_argument(
            "from_slot is behind the retention window",
        )),
        ScriptedSession::closes_empty(),
        ScriptedSession::closes_empty(),
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
    .expect("the budget must run out rather than loop on a refused replay");

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(
        server.resume_points().get(..3),
        Some([None, Some(8), None].as_slice()),
        "the refused replay is given up, not repeated: the attempt after it \
         starts from the live edge"
    );
}

/// ⚠️ **And a stream that opened, delivered nothing and closed gives the replay
/// point up too** — again the same rule as the arm above, on the clean-EOF
/// side. The mark may be past the server's retention, so it is asked for once
/// and then abandoned for the live edge; keeping it loops on a request that
/// cannot succeed, at one attempt per backoff, for the whole budget.
///
/// Found by review of this change, 16 September 2026, with its twin above.
///
/// Mutation this is written against: the same one its `Failed`-side twin names
/// — `Attempt::next_resume_from`'s undelivered branch returning `held` rather
/// than `None`. The branch is shared; the ending that reaches it is not, and
/// that is what this test adds.
///
/// ⚠️ **The mark is produced by the `Failed` arm, on purpose**, though the rule
/// under test is the clean-EOF one. Observing a mark being dropped needs a mark
/// to exist first, and the only producers are the two delivered arms — so
/// making the churn arm produce it would put *its* rule under this test too,
/// and a review of the first version of this file caught exactly that: one
/// mutation reddening two tests sends the reader to the wrong file. How the
/// mark got there is scaffolding; which arm gives it up is the subject.
///
/// ⚠️ A **prefix**, for the reason its `Failed`-side twin gives: reaching a
/// third attempt needs the budget reset of another rule, and breaking *that*
/// one would otherwise make this test red for a defect it does not own.
#[tokio::test]
async fn a_clean_close_that_delivered_nothing_gives_up_the_replay_point() {
    let server = test_geyser_server::start(vec![
        ScriptedSession::Stream(vec![
            routable(10),
            Action::Fail(Status::unavailable("the provider restarted")),
        ]),
        ScriptedSession::closes_empty(),
        ScriptedSession::closes_empty(),
        ScriptedSession::closes_empty(),
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
    .expect("the budget must run out rather than loop on a stale replay point");

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(
        server.resume_points().get(..3),
        Some([None, Some(8), None].as_slice()),
        "the attempt that delivered nothing drops the mark: the one after it \
         starts from the live edge, not from slot 8 again"
    );
}

/// ⚠️ **A stop request must reach a listener parked on a silent stream.** A
/// Yellowstone stream is idle for as long as nothing matches the filters, which
/// is most of the time, so this is where the process spends its life — and a
/// `select!` that did not poll the token there would ignore graceful shutdown
/// for exactly as long as the quiet lasted.
///
/// Mutation this is written against: removing the `shutdown.cancelled()` branch
/// from `connect_and_stream`'s `select!`. It cannot produce a wrong value, only
/// a listener that never returns — which is why the deadline is the assertion.
#[tokio::test]
async fn a_shutdown_reaches_a_listener_parked_on_a_silent_stream() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![Action::Hold])]).await;
    let (downstream, _consumer) = mpsc::channel(4);
    let shutdown = CancellationToken::new();

    let listener = listener_for(&server, 1).await;
    let running = tokio::spawn(listener.run(downstream, shutdown.clone()));

    // Not a sleep: the stream has to be open before the token fires, or this
    // would be testing the `is_cancelled` check at the top of the loop instead.
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

/// ⚠️ **And it must reach one parked inside `handle`, on a full consumer.**
/// `StreamSession::handle` runs in the *body* of the `select!` arm, not as a
/// branch, so while it waits for room nothing polls the token. A consumer that
/// stalls without dropping its receiver would otherwise make the process ignore
/// a stop request for as long as the stall lasts.
///
/// Mutation this is written against: `session.rs`'s back-pressure wait made
/// unconditional — `self.downstream.send(ingested).await` with no `select!`.
/// `session_tests` covers the session's own answer; what is proved here is that
/// the listener turns it into a clean stop rather than another attempt.
///
/// The shape is load-bearing, and it is `session_tests`': **two transactions in
/// the same slot**, because a block-meta releases a slot's payloads in one
/// call, so the second is the one that meets a full channel. Waiting for the
/// first to land is what makes the park certain rather than likely — nobody
/// consumes, so the second can never proceed.
#[tokio::test]
async fn a_shutdown_reaches_a_session_parked_on_a_full_consumer() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![
        routable(10),
        routable(10),
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

/// ⚠️ **A consumer that is gone ends the listener, and is not retried.** There
/// is nothing left to feed and reconnecting would not bring it back, so this is
/// a clean stop — the one ending that is neither churn nor a failing provider.
///
/// Mutation this is written against: treating `DownstreamClosed` as churn.
/// That turns the clean stop into a reconnection, which the subscription count
/// is what names — the budget would then run out on the exhausted script and
/// `run` would return an error instead of `Ok`.
#[tokio::test]
async fn a_vanished_consumer_stops_the_listener_without_retrying() {
    let server = test_geyser_server::start(vec![ScriptedSession::Stream(vec![
        routable(10),
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
