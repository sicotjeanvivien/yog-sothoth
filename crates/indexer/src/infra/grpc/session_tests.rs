//! Tests for what one update does.
//!
//! ⚠️ **The same caveat as `transaction_adapter_tests`, and it is worth
//! repeating rather than referencing:** the updates below are built by hand, so
//! they carry this author's understanding of what a provider sends. What they
//! *can* establish is everything this module decides on its own — which slot a
//! transaction waits for, what happens when a block-meta brings no time, what a
//! ping is answered with, what a full channel does. Those are our rules, not the
//! wire's, and each of them is invisible when wrong.

use super::*;

use chrono::{DateTime, Utc};
use tokio_util::sync::CancellationToken;
use yellowstone_grpc_proto::prelude::{
    Message, SubscribeUpdatePing, SubscribeUpdatePong, SubscribeUpdateTransactionInfo, Transaction,
    TransactionStatusMeta, UnixTimestamp,
};

use crate::infra::grpc::subscription::BLOCK_META_FILTER;

const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("a valid instant")
}

/// A transaction update the adapter accepts: a 64-byte signature, a message
/// with one account key, and a meta that says its inner instructions *were*
/// captured (there simply are none).
fn transaction_update(slot: u64) -> SubscribeUpdateTransaction {
    transaction_update_with_signature(slot, vec![7; 64])
}

fn transaction_update_with_signature(slot: u64, signature: Vec<u8>) -> SubscribeUpdateTransaction {
    SubscribeUpdateTransaction {
        slot,
        transaction: Some(SubscribeUpdateTransactionInfo {
            signature,
            index: 3,
            transaction: Some(Transaction {
                message: Some(Message {
                    account_keys: vec![vec![1; 32]],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            meta: Some(TransactionStatusMeta {
                inner_instructions_none: false,
                ..Default::default()
            }),
            ..Default::default()
        }),
    }
}

fn transaction(slot: u64, filters: &[&str]) -> SubscribeUpdate {
    update(filters, UpdateOneof::Transaction(transaction_update(slot)))
}

fn block_meta(slot: u64, block_time: Option<i64>) -> SubscribeUpdate {
    update(
        &[BLOCK_META_FILTER],
        UpdateOneof::BlockMeta(SubscribeUpdateBlockMeta {
            slot,
            block_time: block_time.map(|timestamp| UnixTimestamp { timestamp }),
            ..Default::default()
        }),
    )
}

fn update(filters: &[&str], oneof: UpdateOneof) -> SubscribeUpdate {
    SubscribeUpdate {
        filters: filters.iter().map(|f| f.to_string()).collect(),
        update_oneof: Some(oneof),
        ..Default::default()
    }
}

/// A session, its downstream receiver, and its outbound receiver.
fn session(
    capacity: usize,
) -> (
    StreamSession,
    mpsc::Receiver<IngestedTransaction>,
    mpsc::Receiver<SubscribeRequest>,
) {
    session_with(capacity, CancellationToken::new())
}

/// The same, with a token the test can fire — for the one case where the
/// session is parked and shutdown has to reach it.
fn session_with(
    capacity: usize,
    shutdown: CancellationToken,
) -> (
    StreamSession,
    mpsc::Receiver<IngestedTransaction>,
    mpsc::Receiver<SubscribeRequest>,
) {
    let (downstream_tx, downstream_rx) = mpsc::channel(capacity);
    let (outbound_tx, outbound_rx) = mpsc::channel(4);
    // A request with both halves that matter to the ping answer: a filter,
    // which must survive it, and a `from_slot`, which must not.
    let request = SubscribeRequest {
        from_slot: Some(99),
        transactions: std::collections::HashMap::from([(
            PROTOCOL.as_str().to_string(),
            Default::default(),
        )]),
        ..Default::default()
    };
    (
        StreamSession::new(downstream_tx, outbound_tx, request, shutdown),
        downstream_rx,
        outbound_rx,
    )
}

// ── the two halves meet ─────────────────────────────────────────────

/// The path the whole slice exists for: a transaction arrives with no instant,
/// waits, and leaves carrying the one its block-meta brought.
#[tokio::test]
async fn a_transaction_waits_for_its_block_time_and_leaves_with_it() {
    let (mut session, mut downstream, _outbound) = session(4);

    assert_eq!(
        session.handle(transaction(10, &[PROTOCOL.as_str()])).await,
        SessionState::Open
    );
    assert!(
        downstream.try_recv().is_err(),
        "nothing may be emitted before an instant is known — the timestamp is \
         a unique-key member and the partitioning column"
    );

    session.handle(block_meta(10, Some(1_700_000_000))).await;

    let ingested = downstream.try_recv().expect("released by its block-meta");
    assert_eq!(
        ingested.protocol, PROTOCOL,
        "read from the filter that matched"
    );
    assert_eq!(ingested.transaction.position.timestamp, at(1_700_000_000));
    assert_eq!(ingested.transaction.position.slot, 10);
    assert_eq!(
        ingested.transaction.position.transaction_index,
        Some(3),
        "the field this migration is about must survive the trip"
    );
}

/// The reverse order, and the one a buffer is easy to write without: the
/// block-meta first, the transaction after.
#[tokio::test]
async fn a_transaction_arriving_after_its_block_meta_leaves_at_once() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(block_meta(10, Some(1_700_000_000))).await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    let ingested = downstream.try_recv().expect("its time was already known");
    assert_eq!(ingested.transaction.position.timestamp, at(1_700_000_000));
}

/// ⚠️ **A block-meta with no `block_time` gives the slot up — it does not leave
/// it waiting, and it does not invent a time.** Neither the receive time nor
/// `SubscribeUpdate::created_at` (the server's send time) is a substitute: this
/// column is in every event table's unique key, so a plausible wrong value is
/// one nothing will ever question.
#[tokio::test]
async fn a_block_meta_without_a_time_gives_up_the_slot() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(block_meta(10, None)).await;

    assert!(
        downstream.try_recv().is_err(),
        "no instant, no row — and no invented one"
    );

    // And the slot is gone rather than pending: a later time for it releases
    // nothing, which is what frees its place in the window.
    session.handle(block_meta(10, Some(1_700_000_000))).await;
    assert!(
        downstream.try_recv().is_err(),
        "the slot was given up, not kept waiting"
    );
}

// ── routing and skip-and-log ────────────────────────────────────────

/// A transaction that matched no protocol filter cannot be routed — the
/// pipeline is per-protocol all the way down. Dropped, not guessed at.
#[tokio::test]
async fn a_transaction_matching_no_protocol_filter_is_dropped() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(transaction(10, &["something_else"])).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;

    assert!(downstream.try_recv().is_err());
}

/// ⚠️ Skip-and-log, and the second half is what this asserts: a malformed
/// transaction must not take the stream down with it, and the next good one
/// must still get through. A `?` in the emit path would pass the first half of
/// this test and fail the second.
#[tokio::test]
async fn a_transaction_that_cannot_be_translated_does_not_stop_the_stream() {
    let (mut session, mut downstream, _outbound) = session(4);

    // 63 bytes: not a signature.
    let malformed = update(
        &[PROTOCOL.as_str()],
        UpdateOneof::Transaction(transaction_update_with_signature(10, vec![7; 63])),
    );
    assert_eq!(session.handle(malformed).await, SessionState::Open);
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;

    let ingested = downstream.try_recv().expect("the sound one still arrives");
    assert_eq!(ingested.transaction.position.slot, 10);
    assert!(
        downstream.try_recv().is_err(),
        "and only the sound one — the malformed transaction is not emitted \
         with some default"
    );
}

/// ⚠️ **The two drops are counted apart, and that is the whole point of the
/// counter's shape.** A malformed message is fixed in the adapter; a
/// transaction matching no protocol filter is fixed in the subscription —
/// nothing about it is malformed, the request and this reader simply disagree.
/// One label for both would send whoever reads the metric to the wrong file,
/// which is the defect `EvictionReason` was added to the buffer to avoid.
///
/// Not `#[tokio::test]`: `with_local_recorder` installs the recorder on the
/// *current thread* for the duration of a closure, so the future is driven
/// inside it — the recipe the persistor tests use.
#[test]
fn a_malformed_transaction_and_an_unroutable_one_are_counted_apart() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                let (mut session, _downstream, _outbound) = session(4);

                // Malformed: a 63-byte signature, which the adapter refuses.
                session
                    .handle(update(
                        &[PROTOCOL.as_str()],
                        UpdateOneof::Transaction(transaction_update_with_signature(
                            10,
                            vec![7; 63],
                        )),
                    ))
                    .await;
                session.handle(block_meta(10, Some(1_700_000_000))).await;

                // Unroutable: perfectly well-formed, matching no protocol.
                session.handle(transaction(11, &["something_else"])).await;
            });
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        dropped_for(&snapshot, "parse_error"),
        Some(&DebugValue::Counter(1)),
        "the malformed one is an adapter problem"
    );
    assert_eq!(
        dropped_for(&snapshot, "unroutable"),
        Some(&DebugValue::Counter(1)),
        "the unroutable one is a subscription problem, and must not hide under \
         the adapter's label"
    );
}

/// The drop counter for one `reason` label, or `None` when it was never
/// touched.
fn dropped_for<'a>(
    snapshot: &'a [(
        metrics_util::CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        metrics_util::debugging::DebugValue,
    )],
    reason: &str,
) -> Option<&'a metrics_util::debugging::DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_grpc_dropped_transactions_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "reason" && l.value() == reason)
        })
        .map(|(_, _, _, value)| value)
}

// ── back-pressure ───────────────────────────────────────────────────

/// ⚠️ **The stream is slowed, never drained into the void** — the opposite of
/// the JSON-RPC dispatcher, which drops when its channel is full. There a
/// dropped signature can be asked for again; here the transaction came once,
/// over a stream billed by the byte.
///
/// ⚠️ The shape of this test is load-bearing and its first version was wrong:
/// with one transaction per slot the consumer emptied the channel between two
/// block-metas, so `try_send` always succeeded and the waiting branch was never
/// entered — a mutation replacing the wait by a drop kept it green. Two
/// transactions in the **same** slot are what makes the second one meet a full
/// channel, since a block-meta releases a slot's payloads in one call.
#[tokio::test]
async fn a_full_downstream_slows_the_stream_instead_of_dropping() {
    // Room for exactly one.
    let (mut session, mut downstream, _outbound) = session(1);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    // This blocks partway through, on the second payload, until the consumer
    // takes the first — which is the behaviour under test.
    let handling = tokio::spawn(async move {
        session.handle(block_meta(10, Some(1_700_000_000))).await;
        session
    });

    let first = recv(&mut downstream).await.expect("the first fits");
    assert_eq!(first.transaction.position.slot, 10);

    let second = recv(&mut downstream)
        .await
        .expect("the second waited for room; dropping it here is the defect");
    assert_eq!(second.transaction.position.slot, 10);

    // Slot 10 *was* closed by its block-meta above, so this is the
    // `highest_meta_slot` branch of `resume_from`, rewound. The pending branch
    // is the business of
    // `the_resume_point_is_the_oldest_slot_the_session_did_not_finish` — said
    // here because an earlier version of this comment claimed the wrong branch,
    // and both happen to yield 8.
    assert_eq!(
        handling.await.expect("no panic").resume_from(),
        Some(10 - REWIND_SLOTS)
    );
}

/// Receive with a deadline: the point of the test above is that a transaction
/// *arrives*, and a version of the code that drops it would otherwise hang the
/// suite instead of failing it.
async fn recv(downstream: &mut mpsc::Receiver<IngestedTransaction>) -> Option<IngestedTransaction> {
    tokio::time::timeout(std::time::Duration::from_secs(5), downstream.recv())
        .await
        .expect("nothing arrived within five seconds")
}

/// A consumer that is gone ends the session: there is nothing left to feed, and
/// retrying the connection would not bring it back.
#[tokio::test]
async fn a_closed_downstream_ends_the_session() {
    let (mut session, downstream, _outbound) = session(4);
    drop(downstream);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    assert_eq!(
        session.handle(block_meta(10, Some(1_700_000_000))).await,
        SessionState::DownstreamClosed
    );
}

// ── keep-alive ──────────────────────────────────────────────────────

/// ⚠️ **A ping is counted and not answered**, and every alternative is unsafe
/// under one of the two readings of the proto — see the note on
/// `StreamSession`. This test is what keeps an "obvious improvement" from
/// quietly re-introducing one: answering with the request re-issues the replay
/// every ping, answering without it truncates a replay in flight, and answering
/// with a bare ping may unsubscribe everything.
#[tokio::test]
async fn a_ping_is_counted_and_nothing_is_sent_back() {
    let (mut session, _downstream, mut outbound) = session(4);

    assert_eq!(
        session
            .handle(update(&[], UpdateOneof::Ping(SubscribeUpdatePing {})))
            .await,
        SessionState::Open
    );

    assert!(
        outbound.try_recv().is_err(),
        "nothing goes back on the outbound half — the connection is kept alive \
         one layer down, by HTTP/2 keep-alive"
    );
}

/// A pong is the answer to one of ours: counted, and nothing more.
#[tokio::test]
async fn a_pong_changes_nothing() {
    let (mut session, mut downstream, mut outbound) = session(4);

    assert_eq!(
        session
            .handle(update(
                &[],
                UpdateOneof::Pong(SubscribeUpdatePong { id: 1 })
            ))
            .await,
        SessionState::Open
    );

    assert!(downstream.try_recv().is_err());
    assert!(
        outbound.try_recv().is_err(),
        "nothing is sent back for a pong either"
    );
}

// ── resuming ────────────────────────────────────────────────────────

/// ⚠️ **Where a reconnection resumes from is the oldest slot this session did
/// not finish — not the highest slot it saw.** The first version took the
/// highest slot of *any* update, transaction updates included, and that lost
/// data on every mid-block break: a transaction names a slot still in flight,
/// its payloads sit in the buffer, and the buffer dies with the session. Asking
/// for `M+1` then skipped exactly what the break destroyed. Found in review,
/// 9 September 2026, and this test is the shape of that defect.
#[tokio::test]
async fn the_resume_point_is_the_oldest_slot_the_session_did_not_finish() {
    let (mut session, _downstream, _outbound) = session(4);

    assert_eq!(session.resume_from(), None, "nothing arrived yet");

    // Slot 10 is closed by its block-meta: finished.
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;
    assert_eq!(
        session.resume_from(),
        Some(10 - REWIND_SLOTS),
        "the last closed slot, rewound — a block-meta does not promise its \
         slot's transactions have all arrived"
    );

    // Slot 12 arrives and is still in flight when the break comes.
    session.handle(transaction(12, &[PROTOCOL.as_str()])).await;
    assert_eq!(
        session.resume_from(),
        Some(12 - REWIND_SLOTS),
        "slot 12's payloads die with this session, so the replay must cover \
         them — asking for 13 would drop them silently"
    );
}

/// The rewind must not underflow near genesis, which is only reachable in a
/// test but is the kind of arithmetic that panics in release-mode debug builds
/// and wraps in release.
#[tokio::test]
async fn the_rewind_saturates_instead_of_wrapping() {
    let (mut session, _downstream, _outbound) = session(4);

    session.handle(transaction(1, &[PROTOCOL.as_str()])).await;

    assert_eq!(session.resume_from(), Some(0));
}

/// ⚠️ **What separates churn from a provider refusing us — and a ping is not
/// it.** A stream that opens and closes having delivered no data must count
/// against the retry budget; one that delivered must not. Yellowstone servers
/// ping shortly after `subscribe`, so counting "any message" would put a stream
/// that pings once and closes in the churn arm: counter reset, redialled once a
/// second, for ever. Found in review, 9 September 2026 — the day after the rule
/// this test was first written for.
#[tokio::test]
async fn only_data_counts_as_delivered_not_a_keep_alive() {
    let (mut session, _downstream, _outbound) = session(4);

    assert!(!session.received_data(), "nothing came off the stream");

    session
        .handle(update(&[], UpdateOneof::Ping(SubscribeUpdatePing {})))
        .await;
    assert!(
        !session.received_data(),
        "a keep-alive is not delivery — it is what a refusing server sends \
         before closing"
    );

    session
        .handle(update(
            &[],
            UpdateOneof::Pong(SubscribeUpdatePong { id: 1 }),
        ))
        .await;
    assert!(
        !session.received_data(),
        "nor is the answer to our own ping"
    );

    session.handle(block_meta(10, Some(1_700_000_000))).await;
    assert!(session.received_data(), "a block-meta is data");
}

/// ⚠️ **The back-pressure wait must not swallow a shutdown.** `handle` is driven
/// from the body of the listener's `select!` arm, so while it is parked on a
/// full consumer nothing else polls the cancellation token. A consumer that
/// stalls without dropping its receiver would otherwise make the process ignore
/// a stop request for as long as the stall lasts.
#[tokio::test]
async fn a_shutdown_reaches_a_session_parked_on_a_full_consumer() {
    let shutdown = CancellationToken::new();
    // Room for one, two payloads in the slot: the second parks.
    let (mut session, _downstream, _outbound) = session_with(1, shutdown.clone());

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    let handling =
        tokio::spawn(async move { session.handle(block_meta(10, Some(1_700_000_000))).await });

    // Nobody consumes; the stop request is what has to get through.
    shutdown.cancel();

    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), handling)
            .await
            .expect("the wait must end when the token fires")
            .expect("no panic"),
        SessionState::ShutdownRequested
    );
}
