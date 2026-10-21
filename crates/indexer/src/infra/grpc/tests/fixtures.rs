//! What the gRPC tests share: the updates they put on the wire, and the metrics snapshot.
//! ⚠️ They carry our reading of the proto, not a provider's behaviour.
//! ⚠️ [`transaction_update`] must stay one the adapter accepts: a refused one is dropped before
//! the consumer, and the full- and vanished-consumer tests would exercise nothing.

use chrono::{DateTime, Utc};
use yellowstone_grpc_proto::prelude::{
    Message, SubscribeUpdate, SubscribeUpdateBlockMeta, SubscribeUpdatePing,
    SubscribeUpdateTransaction, SubscribeUpdateTransactionInfo, Transaction, TransactionStatusMeta,
    UnixTimestamp, subscribe_update::UpdateOneof,
};
use yog_core::domain::Protocol;

use super::subscription::BLOCK_META_FILTER;

/// The one protocol with a working extractor.
pub(super) const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

/// What the metrics recorder holds after a run.
pub(super) type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    metrics_util::debugging::DebugValue,
)>;

pub(super) fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("a valid instant")
}

/// A transaction update the adapter accepts: a 64-byte signature, one account key, and a meta
/// whose inner instructions were captured (there are none).
pub(super) fn transaction_update(slot: u64) -> SubscribeUpdateTransaction {
    transaction_update_with_signature(slot, vec![7; 64])
}

pub(super) fn transaction_update_with_signature(
    slot: u64,
    signature: Vec<u8>,
) -> SubscribeUpdateTransaction {
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
        bank_id: 0,
    }
}

/// The filter name no protocol claims, spelled once for every fixture.
/// ⚠️ `subscription_tests` keeps its own literal: `protocol_of`'s contract is any unknown name.
pub(super) const UNROUTABLE_FILTER: &str = "a_filter_no_protocol_claims";

/// A transaction the pipeline cannot route: delivery with nothing to resume from.
/// ⚠️ A conformant server never sends it (`build_request` names filters `Protocol::as_str`): it is
/// non-conformant on purpose, not evidence of frequency.
pub(super) fn unroutable_transaction(slot: u64) -> SubscribeUpdate {
    transaction(slot, &[UNROUTABLE_FILTER])
}

/// A transaction update wrapped in the filters it matched.
/// ⚠️ A name that is no protocol makes it unroutable, dropped before the buffer.
pub(super) fn transaction(slot: u64, filters: &[&str]) -> SubscribeUpdate {
    update(filters, UpdateOneof::Transaction(transaction_update(slot)))
}

pub(super) fn block_meta(slot: u64, block_time: Option<i64>) -> SubscribeUpdate {
    update(
        &[BLOCK_META_FILTER],
        UpdateOneof::BlockMeta(SubscribeUpdateBlockMeta {
            slot,
            block_time: block_time.map(|timestamp| UnixTimestamp { timestamp }),
            ..Default::default()
        }),
    )
}

/// A server keep-alive, the one message that is not data: `received_data` must stay false.
pub(super) fn ping() -> SubscribeUpdate {
    update(&[], UpdateOneof::Ping(SubscribeUpdatePing {}))
}

pub(super) fn update(filters: &[&str], oneof: UpdateOneof) -> SubscribeUpdate {
    SubscribeUpdate {
        filters: filters.iter().map(|f| f.to_string()).collect(),
        update_oneof: Some(oneof),
        ..Default::default()
    }
}
