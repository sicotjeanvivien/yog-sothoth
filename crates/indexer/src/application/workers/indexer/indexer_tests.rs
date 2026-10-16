use super::*;
use chrono::Utc;
use solana_signature::Signature;
use std::time::Duration;
use yog_core::{
    application::extraction::OnChainTransaction,
    domain::{Protocol, TransactionPosition},
};

fn ingested(slot: u64) -> IngestedTransaction {
    IngestedTransaction {
        protocol: Protocol::MeteoraDammV2,
        transaction: OnChainTransaction {
            position: TransactionPosition {
                signature: Signature::from([0u8; 64]),
                timestamp: Utc::now(),
                slot,
                transaction_index: None,
            },
            inner_instructions: Vec::new(),
        },
    }
}

#[tokio::test]
async fn the_drain_counts_every_transaction_left_in_the_channel() {
    let (tx, mut rx) = mpsc::channel::<IngestedTransaction>(8);
    for slot in 0..5 {
        tx.send(ingested(slot)).await.expect("channel has room");
    }

    assert_eq!(
        drain_and_count(&mut rx),
        5,
        "a shutdown with five transactions queued must report five, not zero — \
         a drain that stops early is indistinguishable from an empty channel"
    );
}

#[tokio::test]
async fn an_empty_channel_drains_to_zero() {
    // The case that must stay silent: no log line, no metric, no invented loss.
    let (_tx, mut rx) = mpsc::channel::<IngestedTransaction>(8);
    assert_eq!(drain_and_count(&mut rx), 0);
}

// ── Waiting for what was detached ────────────────────────────────────────────

/// ⚠️ The stop must be last: nothing holds a detached task's handle, so a permit still out is the
/// only evidence of a write. Mutation this is written against: dropping the `acquire_many`.
#[tokio::test(start_paused = true)]
async fn the_stop_waits_for_indexing_that_is_still_in_flight() {
    let semaphore = Arc::new(Semaphore::new(4));
    let held = Arc::clone(&semaphore)
        .acquire_owned()
        .await
        .expect("permits are free");

    let waiting = tokio::spawn({
        let semaphore = Arc::clone(&semaphore);
        async move { await_in_flight(&semaphore, 4).await }
    });

    // The paused clock jumps this second when the runtime idles, when an early wait would return.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(
        !waiting.is_finished(),
        "the stop came back while a transaction was still being written"
    );

    drop(held);
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .expect("the wait must end once the last permit is back")
        .expect("the waiting task must not panic");
}

/// The other half: with nothing in flight there is nothing to wait for, not a hang.
#[tokio::test]
async fn a_stop_with_nothing_in_flight_does_not_wait() {
    let semaphore = Semaphore::new(4);

    tokio::time::timeout(Duration::from_secs(5), await_in_flight(&semaphore, 4))
        .await
        .expect("an idle stage must not block the shutdown");
}
