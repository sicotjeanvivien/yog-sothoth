use super::*;
use chrono::Utc;
use solana_signature::Signature;
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
