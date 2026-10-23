//! Tests for what one update does.
//! ⚠️ The updates are hand-built (`test_fixtures`): these tests prove this module's own rules,
//! not what a provider sends.

use super::*;

use super::{ping_answer::PING_REPLY_ID, slot_progress::REWIND_SLOTS};

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use tokio_util::sync::CancellationToken;
use yellowstone_grpc_proto::prelude::{
    SlotStatus, SubscribeRequestPing, SubscribeUpdatePing, SubscribeUpdatePong,
};

use crate::infra::grpc::test_fixtures::{
    PROTOCOL, Snapshot, at, block_meta, block_meta_in_bank, gauge_for, ping, slot_finalized,
    slot_status, transaction, transaction_in_bank, transaction_update_with_signature,
    unroutable_transaction, update,
};

/// How many ping answers the harness's outbound half holds unread.
const HARNESS_OUTBOUND_CAPACITY: usize = 4;

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

/// The same, with a token the test can fire, for a session parked when shutdown comes.
fn session_with(
    capacity: usize,
    shutdown: CancellationToken,
) -> (
    StreamSession,
    mpsc::Receiver<IngestedTransaction>,
    mpsc::Receiver<SubscribeRequest>,
) {
    let (downstream_tx, downstream_rx) = mpsc::channel(capacity);
    let (outbound_tx, outbound_rx) = mpsc::channel(HARNESS_OUTBOUND_CAPACITY);
    // Only ping answers are ever written here; the receiver is handed back to assert them.
    (
        StreamSession::new(downstream_tx, outbound_tx, shutdown),
        downstream_rx,
        outbound_rx,
    )
}

// ── the two halves meet ─────────────────────────────────────────────

/// A transaction arrives with no instant, waits, and leaves with the one its block-meta brought.
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

    session.handle(slot_finalized(10, None)).await;
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

/// The reverse order: the block-meta first, the transaction after.
#[tokio::test]
async fn a_transaction_arriving_after_its_block_meta_leaves_at_once() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(slot_finalized(10, None)).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    let ingested = downstream.try_recv().expect("its time was already known");
    assert_eq!(ingested.transaction.position.timestamp, at(1_700_000_000));
}

/// ⚠️ A block-meta with no `block_time` gives the slot up: neither the receive time nor
/// `created_at` substitutes, in a column of every unique key.
#[tokio::test]
async fn a_block_meta_without_a_time_gives_up_the_slot() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(slot_finalized(10, None)).await;
    session.handle(block_meta(10, None)).await;

    assert!(
        downstream.try_recv().is_err(),
        "no instant, no row — and no invented one"
    );

    // The slot is gone rather than pending: a later time for it releases nothing.
    session.handle(block_meta(10, Some(1_700_000_000))).await;
    assert!(
        downstream.try_recv().is_err(),
        "the slot was given up, not kept waiting"
    );
}

// ── banks ───────────────────────────────────────────────────────────

const BANK_ID_MISSING: &str = "yog_indexer_grpc_bank_id_missing";

/// Drain what reached the consumer, by signature's first byte.
fn drain_signatures(downstream: &mut mpsc::Receiver<IngestedTransaction>) -> Vec<u8> {
    std::iter::from_fn(|| downstream.try_recv().ok())
        .map(|ingested| ingested.transaction.position.signature.as_ref()[0])
        .collect()
}

/// Alchemy's shape: nothing names a bank. Against a fallback that drops the slot, or a gauge left unset.
#[test]
fn a_status_naming_no_bank_pairs_its_slot_by_slot_and_raises_the_gauge() {
    let (delivered, snapshot) = counted(async {
        let (mut session, mut downstream, _outbound) = session(4);
        session.handle(slot_finalized(10, None)).await;
        session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
        session.handle(block_meta(10, Some(1_700_000_000))).await;
        drain_signatures(&mut downstream).len()
    });

    assert_eq!(delivered, 1);
    assert_eq!(gauge_for(&snapshot, BANK_ID_MISSING), Some(1.0));
}

/// Against a key back to the slot alone: the transaction of bank 6 would reach the consumer.
#[test]
fn a_server_naming_banks_lets_only_the_kept_banks_transactions_through() {
    let (delivered, snapshot) = counted(async {
        let (mut session, mut downstream, _outbound) = session(4);
        session
            .handle(transaction_in_bank(10, 5, vec![5; 64]))
            .await;
        session
            .handle(transaction_in_bank(10, 6, vec![6; 64]))
            .await;
        session
            .handle(block_meta_in_bank(10, 5, Some(1_700_000_000)))
            .await;
        session
            .handle(block_meta_in_bank(10, 6, Some(1_700_000_001)))
            .await;
        session.handle(slot_finalized(10, Some(5))).await;
        drain_signatures(&mut downstream)
    });

    assert_eq!(delivered, vec![5], "bank 5's transaction alone");
    assert_eq!(
        gauge_for(&snapshot, BANK_ID_MISSING),
        Some(0.0),
        "every message named its bank"
    );
}

/// Against a 0 read as a bank of its own: every transaction of a half-filling server would be
/// dropped as losing. The bank-less message also raises the gauge for the rest of the session.
#[test]
fn a_transaction_naming_bank_0_is_paired_with_the_kept_bank() {
    let (delivered, snapshot) = counted(async {
        let (mut session, mut downstream, _outbound) = session(4);
        session.handle(slot_finalized(10, Some(5))).await;
        session
            .handle(transaction_in_bank(10, 0, vec![7; 64]))
            .await;
        session
            .handle(block_meta_in_bank(10, 5, Some(1_700_000_000)))
            .await;
        drain_signatures(&mut downstream)
    });

    assert_eq!(delivered, vec![7]);
    assert_eq!(gauge_for(&snapshot, BANK_ID_MISSING), Some(1.0));
}

/// Against a settlement on any status: a `Confirmed` one would let out what `finalized` holds.
#[tokio::test]
async fn a_status_other_than_finalized_releases_nothing() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session
        .handle(slot_status(10, SlotStatus::SlotConfirmed, None))
        .await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;

    assert!(downstream.try_recv().is_err());
}

/// Against a status counted as `other`, the label that says the request and the reader disagree.
#[test]
fn a_status_is_data_counted_under_its_own_kind() {
    let (received, snapshot) = counted(async {
        let (mut session, _downstream, _outbound) = session(4);
        session.handle(slot_finalized(10, None)).await;
        session.received_data()
    });

    assert!(
        received,
        "a status is the server delivering, not a keep-alive"
    );
    let kind = |kind: &str| {
        snapshot.iter().find_map(|(key, _, _, value)| {
            (key.key().name() == "yog_indexer_grpc_updates_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "kind" && l.value() == kind))
            .then_some(value)
        })
    };
    assert_eq!(kind("slot"), Some(&DebugValue::Counter(1)));
    assert_eq!(kind("other"), None);
}

// ── routing and skip-and-log ────────────────────────────────────────

/// A transaction that matched no protocol filter is dropped, not guessed at.
#[tokio::test]
async fn a_transaction_matching_no_protocol_filter_is_dropped() {
    let (mut session, mut downstream, _outbound) = session(4);

    session.handle(unroutable_transaction(10)).await;
    session.handle(slot_finalized(10, None)).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;

    assert!(downstream.try_recv().is_err());
}

/// ⚠️ Skip-and-log: a `?` in the emit path would pass the first half of this test and fail the
/// second, the next good transaction getting through.
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
    session.handle(slot_finalized(10, None)).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;

    let ingested = downstream.try_recv().expect("the sound one still arrives");
    assert_eq!(ingested.transaction.position.slot, 10);
    assert!(
        downstream.try_recv().is_err(),
        "and only the sound one — the malformed transaction is not emitted \
         with some default"
    );
}

/// ⚠️ The two drops are counted apart: a malformed message is fixed in the adapter, an unroutable
/// one in the subscription. Not `#[tokio::test]`, see [`counted`].
#[test]
fn a_malformed_transaction_and_an_unroutable_one_are_counted_apart() {
    let ((), snapshot) = counted(async {
        let (mut session, _downstream, _outbound) = session(4);

        // Malformed: a 63-byte signature, which the adapter refuses.
        session
            .handle(update(
                &[PROTOCOL.as_str()],
                UpdateOneof::Transaction(transaction_update_with_signature(10, vec![7; 63])),
            ))
            .await;
        session.handle(slot_finalized(10, None)).await;
        session.handle(block_meta(10, Some(1_700_000_000))).await;

        // Unroutable: perfectly well-formed, matching no protocol.
        session.handle(unroutable_transaction(11)).await;
    });

    assert_eq!(
        counter_for(&snapshot, DROPPED, "parse_error"),
        Some(&DebugValue::Counter(1)),
        "the malformed one is an adapter problem"
    );
    assert_eq!(
        counter_for(&snapshot, DROPPED, "unroutable"),
        Some(&DebugValue::Counter(1)),
        "the unroutable one is a subscription problem, and must not hide under \
         the adapter's label"
    );
}

const DROPPED: &str = "yog_indexer_grpc_dropped_transactions_total";
const PING_REPLIES_UNSENT: &str = "yog_indexer_grpc_ping_replies_unsent_total";

/// Drive `future` to completion and return its output with what it counted.
/// ⚠️ `with_local_recorder` binds the current thread, hence a current-thread runtime and `#[test]`.
fn counted<T>(future: impl std::future::Future<Output = T>) -> (T, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let output = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("current-thread runtime")
            .block_on(future)
    });

    (output, snapshotter.snapshot().into_vec())
}

/// The counter `name` for one `reason` label, or `None` when never touched.
fn counter_for<'a>(snapshot: &'a Snapshot, name: &str, reason: &str) -> Option<&'a DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| {
            key.key().name() == name
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "reason" && l.value() == reason)
        })
        .map(|(_, _, _, value)| value)
}

// ── back-pressure ───────────────────────────────────────────────────

/// ⚠️ The stream is slowed, never drained into the void: a streamed transaction comes once.
/// ⚠️ Two transactions in the same slot, released in one call: with one per slot the consumer
/// empties the channel between metas and the wait is never entered.
#[tokio::test]
async fn a_full_downstream_slows_the_stream_instead_of_dropping() {
    // Room for exactly one.
    let (mut session, mut downstream, _outbound) = session(1);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    // Blocks on the second payload until the consumer takes the first: the behaviour under test.
    session.handle(slot_finalized(10, None)).await;
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

    // Slot 10 was closed by its block-meta: the `highest_meta_slot` branch of `resume_from`, rewound.
    assert_eq!(
        handling.await.expect("no panic").resume_from(),
        Some(10 - REWIND_SLOTS)
    );
}

/// Receive with a deadline: a dropped transaction must fail the suite, not hang it.
async fn recv(downstream: &mut mpsc::Receiver<IngestedTransaction>) -> Option<IngestedTransaction> {
    tokio::time::timeout(std::time::Duration::from_secs(5), downstream.recv())
        .await
        .expect("nothing arrived within five seconds")
}

/// A consumer that is gone ends the session: retrying the connection would not bring it back.
#[tokio::test]
async fn a_closed_downstream_ends_the_session() {
    let (mut session, downstream, _outbound) = session(4);
    drop(downstream);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    session.handle(slot_finalized(10, None)).await;
    assert_eq!(
        session.handle(block_meta(10, Some(1_700_000_000))).await,
        SessionState::DownstreamClosed
    );
}

// ── keep-alive ──────────────────────────────────────────────────────

/// ⚠️ A ping is answered with a request whose only field is `ping`: a `from_slot` would re-issue
/// the replay on every ping, a filter would describe a subscription anew.
#[tokio::test]
async fn a_ping_is_answered_with_a_bare_ping() {
    let (mut session, _downstream, mut outbound) = session(4);

    assert_eq!(session.handle(ping()).await, SessionState::Open);

    let answer = outbound
        .try_recv()
        .expect("a server ping must be answered — a provider may close a silent client");
    assert!(outbound.try_recv().is_err(), "exactly one answer per ping");

    assert_eq!(
        answer.from_slot, None,
        "a `from_slot` on the answer would re-issue the replay on every ping"
    );
    assert!(
        answer.accounts.is_empty()
            && answer.slots.is_empty()
            && answer.transactions.is_empty()
            && answer.transactions_status.is_empty()
            && answer.blocks.is_empty()
            && answer.blocks_meta.is_empty()
            && answer.entry.is_empty(),
        "a filter on the answer describes a subscription — the answer must carry none: {answer:?}"
    );
    // The net under the two above: every other field, current or future, stays at its default.
    assert_eq!(
        answer,
        SubscribeRequest {
            ping: Some(SubscribeRequestPing { id: PING_REPLY_ID }),
            ..Default::default()
        }
    );
}

/// A ping during a replay leaves the session's resume point and pending payload as they were.
/// ⚠️ This guards `handle`'s `Ping` arm; an answer's damage is [`a_ping_is_answered_with_a_bare_ping`]'s.
#[tokio::test]
async fn a_ping_during_a_replay_leaves_it_alone() {
    let (mut session, mut downstream, mut outbound) = session(4);

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    assert_eq!(session.resume_from(), Some(10 - REWIND_SLOTS));

    assert_eq!(session.handle(ping()).await, SessionState::Open);

    assert!(outbound.try_recv().is_ok(), "the ping is answered");
    assert_eq!(
        session.resume_from(),
        Some(10 - REWIND_SLOTS),
        "a ping does not move the resume point"
    );
    assert!(
        downstream.try_recv().is_err(),
        "a ping releases nothing — the payload is still waiting for its block time"
    );

    session.handle(slot_finalized(10, None)).await;
    session.handle(block_meta(10, Some(1_700_000_000))).await;
    let ingested = downstream
        .try_recv()
        .expect("the payload the ping found pending is still in the buffer");
    assert_eq!(ingested.transaction.position.slot, 10);
    assert_eq!(ingested.transaction.position.timestamp, at(1_700_000_000));
    assert!(downstream.try_recv().is_err(), "and nothing else was held");
}

/// An answer that cannot leave is counted and stepped over: ending the session would turn a
/// failed keep-alive into the reconnection it exists to avoid.
#[test]
fn a_ping_whose_answer_cannot_leave_is_counted_and_keeps_the_session_open() {
    let (state, snapshot) = counted(async {
        let (mut session, _downstream, outbound) = session(4);
        drop(outbound);
        session.handle(ping()).await
    });

    assert_eq!(state, SessionState::Open);
    assert_eq!(
        counter_for(&snapshot, PING_REPLIES_UNSENT, "outbound_closed"),
        Some(&DebugValue::Counter(1)),
        "a request stream that is gone is counted as such"
    );
    assert_eq!(
        counter_for(&snapshot, PING_REPLIES_UNSENT, "outbound_full"),
        None,
        "and not as a transport that stopped draining — the two are fixed in \
         different places"
    );
}

/// ⚠️ A full outbound half must not park the session, which runs in `select!`'s body: the next ping
/// returns at once, its answer dropped and counted.
#[test]
fn a_full_outbound_half_does_not_park_the_session() {
    let ((state, queued), snapshot) = counted(async {
        let (mut session, _downstream, mut outbound) = session(4);

        for _ in 0..HARNESS_OUTBOUND_CAPACITY {
            session.handle(ping()).await;
        }
        let state = tokio::time::timeout(std::time::Duration::from_secs(5), session.handle(ping()))
            .await
            .expect("an answer that does not fit must be dropped, not waited on");

        // Free the queue, give anything parked a chance to run, then count.
        let mut queued = 0;
        while outbound.try_recv().is_ok() {
            queued += 1;
        }
        tokio::task::yield_now().await;
        while outbound.try_recv().is_ok() {
            queued += 1;
        }

        (state, queued)
    });

    assert_eq!(state, SessionState::Open);
    assert_eq!(
        queued, HARNESS_OUTBOUND_CAPACITY,
        "the answer that did not fit is dropped — nothing may hold it to send once there is room"
    );
    assert_eq!(
        counter_for(&snapshot, PING_REPLIES_UNSENT, "outbound_full"),
        Some(&DebugValue::Counter(1)),
        "a transport that stopped draining is counted as such"
    );
    assert_eq!(
        counter_for(&snapshot, PING_REPLIES_UNSENT, "outbound_closed"),
        None,
        "and not as a request stream that is gone"
    );
}

/// A pong, the server's answer to one of our ping answers: counted, nothing more.
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

/// ⚠️ The resume point is the oldest slot not finished, not the highest seen: a mid-block break
/// leaves payloads in the buffer, which dies with the session.
#[tokio::test]
async fn the_resume_point_is_the_oldest_slot_the_session_did_not_finish() {
    let (mut session, _downstream, _outbound) = session(4);

    assert_eq!(session.resume_from(), None, "nothing arrived yet");

    // Slot 10 is closed by its block-meta: finished.
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(slot_finalized(10, None)).await;
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

/// The rewind must not underflow near genesis.
#[tokio::test]
async fn the_rewind_saturates_instead_of_wrapping() {
    let (mut session, _downstream, _outbound) = session(4);

    session.handle(transaction(1, &[PROTOCOL.as_str()])).await;

    assert_eq!(session.resume_from(), Some(0));
}

/// ⚠️ Only data separates churn from a refusal: servers ping right after `subscribe`, so counting
/// a ping would redial a refusing provider once a second for ever.
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

/// ⚠️ Only a block-meta moves the stall clock's count, with or without a time: pings outlive a
/// stopped data path, and a quiet market sends no transaction for minutes.
#[tokio::test]
async fn only_a_block_meta_moves_the_block_meta_count() {
    let (mut session, _downstream, _outbound) = session(4);

    session.handle(ping()).await;
    session
        .handle(update(
            &[],
            UpdateOneof::Pong(SubscribeUpdatePong { id: 1 }),
        ))
        .await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(unroutable_transaction(11)).await;
    session.handle(slot_finalized(10, None)).await;
    assert_eq!(
        session.block_metas_received(),
        0,
        "pings, pongs, statuses and transactions say nothing about whether block-metas still flow"
    );

    session.handle(block_meta(10, Some(1_700_000_000))).await;
    assert_eq!(session.block_metas_received(), 1);

    session.handle(block_meta(11, None)).await;
    assert_eq!(
        session.block_metas_received(),
        2,
        "a block-meta without a time gives its slot up, but the stream did deliver it"
    );
}

/// ⚠️ An unroutable transaction is delivery with nothing to resume from: the premise of the
/// listener's keep-the-mark rule, pinned here so its loss reddens this test.
#[tokio::test]
async fn an_unroutable_transaction_is_delivery_with_nothing_to_resume_from() {
    let (mut session, _downstream, _outbound) = session(4);

    session.handle(unroutable_transaction(10)).await;

    assert!(
        session.received_data(),
        "the server sent data and did not refuse us — the retry budget must not \
         be charged for this session"
    );
    assert_eq!(
        session.resume_from(),
        None,
        "it never reached the buffer, so this session finished no slot and \
         left none pending"
    );
}

/// ⚠️ `handle` runs in `select!`'s body: parked on a full consumer, it must still see shutdown.
#[tokio::test]
async fn a_shutdown_reaches_a_session_parked_on_a_full_consumer() {
    let shutdown = CancellationToken::new();
    // Room for one, two payloads in the slot: the second parks.
    let (mut session, _downstream, _outbound) = session_with(1, shutdown.clone());

    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;
    session.handle(transaction(10, &[PROTOCOL.as_str()])).await;

    session.handle(slot_finalized(10, None)).await;
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
