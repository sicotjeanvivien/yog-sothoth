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
    let (downstream_tx, downstream_rx) = mpsc::channel(capacity);
    let (outbound_tx, outbound_rx) = mpsc::channel(4);
    let request = SubscribeRequest {
        from_slot: Some(99),
        ..Default::default()
    };
    (
        StreamSession::new(downstream_tx, outbound_tx, request),
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

    assert_eq!(handling.await.expect("no panic").highest_slot(), Some(10));
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

/// ⚠️ **A ping is answered with the subscription itself, not with a bare ping.**
/// The proto allows a request to carry a `ping`, and a request is also what
/// describes the subscription — so a ping-only request is either a keep-alive or
/// an unsubscribe-everything, and no reachable endpoint can say which. Resending
/// the request already in force is correct under both readings, and this test is
/// what keeps it that way.
#[tokio::test]
async fn a_ping_is_answered_with_the_subscription_unchanged() {
    let (mut session, _downstream, mut outbound) = session(4);

    session
        .handle(update(&[], UpdateOneof::Ping(SubscribeUpdatePing {})))
        .await;

    let answer = outbound.try_recv().expect("a ping is answered");
    assert!(answer.ping.is_some(), "it must be recognisable as a ping");
    assert_eq!(
        answer.from_slot,
        Some(99),
        "and it must carry the same subscription — an emptied request could be \
         read as unsubscribing from everything"
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
        "answering a pong would be a loop"
    );
}

// ── resuming ────────────────────────────────────────────────────────

/// ⚠️ Where a reconnection resumes from is the **highest** slot seen, from
/// either half of the stream — not the last one to arrive. Updates are not
/// globally ordered between the two subscriptions, so taking the latest arrival
/// would hand `from_slot` a slot already passed, and re-ask for data that was
/// already written.
#[tokio::test]
async fn the_resume_point_is_the_highest_slot_seen_from_either_half() {
    let (mut session, _downstream, _outbound) = session(4);

    assert_eq!(session.highest_slot(), None, "nothing arrived yet");

    session.handle(transaction(12, &[PROTOCOL.as_str()])).await;
    assert_eq!(session.highest_slot(), Some(12));

    // A block-meta for an earlier slot must not move the mark backwards.
    session.handle(block_meta(11, Some(1_700_000_000))).await;
    assert_eq!(session.highest_slot(), Some(12));

    session.handle(block_meta(13, Some(1_700_000_001))).await;
    assert_eq!(session.highest_slot(), Some(13));
}
