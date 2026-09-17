//! The updates a test puts on the wire, built once for the whole path.
//!
//! Two test modules need the same protobuf shapes and for different reasons:
//! `session_tests` feeds them to [`StreamSession::handle`] directly, and
//! `listener_tests` has `test_geyser_server` send them down a real stream so that
//! `GrpcListener::run` sees them arrive. The shapes themselves are the same
//! messages, so they are defined here rather than twice.
//!
//! ⚠️ **They carry this author's reading of the proto, not a provider's
//! behaviour** — the same caveat `transaction_adapter_tests` and
//! `session_tests` already state, and it is worth repeating here because this
//! module is now what both of them rest on. What these fixtures can establish
//! is what *our* code decides when a message of a given shape arrives; what
//! they cannot is that a server sends that shape. That half needs a real
//! provider.
//!
//! ⚠️ And one property below is load-bearing in a way that is easy to undo:
//! [`transaction_update`] produces a transaction the **adapter accepts**. Half
//! of `listener_tests` depends on it — a transaction that `from_grpc` refuses
//! is dropped inside `StreamSession::emit`, which returns `Open`, so the
//! consumer is never reached and the tests that turn on a full or vanished
//! consumer would pass while exercising nothing.
//!
//! [`StreamSession::handle`]: super::session::StreamSession::handle

use chrono::{DateTime, Utc};
use yellowstone_grpc_proto::prelude::{
    Message, SubscribeUpdate, SubscribeUpdateBlockMeta, SubscribeUpdatePing,
    SubscribeUpdateTransaction, SubscribeUpdateTransactionInfo, Transaction, TransactionStatusMeta,
    UnixTimestamp, subscribe_update::UpdateOneof,
};
use yog_core::domain::Protocol;

use super::subscription::BLOCK_META_FILTER;

/// The one protocol with a working extractor, and therefore the only filter
/// name an update can carry and still be routed.
pub(super) const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

pub(super) fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("a valid instant")
}

/// A transaction update the adapter accepts: a 64-byte signature, a message
/// with one account key, and a meta that says its inner instructions *were*
/// captured (there simply are none).
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
    }
}

/// The filter name of a transaction **no protocol claims**, and the one place a
/// fixture spells it.
///
/// It is the other half of [`PROTOCOL`]: that constant is the name that routes,
/// this one is a name that does not, and both are rules rather than strings.
/// Four spellings were in the tree on 16 September 2026 — found in review, over
/// two passes, against the rule the sibling constant's own comment states.
///
/// ⚠️ **`subscription_tests` keeps its own literal, and that is the decision.**
/// It tests `protocol_of` itself, whose contract is that *any* unknown name
/// answers `None` — tying that assertion to this constant would make it prove
/// something narrower than the contract. The rule stated here is what a
/// *fixture* spells, not every string in the crate that happens to name no
/// protocol.
pub(super) const UNROUTABLE_FILTER: &str = "a_filter_no_protocol_claims";

/// A transaction the pipeline cannot route: well-formed, matching no protocol
/// filter.
///
/// ⚠️ **It is delivery with nothing to resume from**, and that pair is why it
/// has a fixture of its own. `protocol_of` answers `None`, so `on_transaction`
/// counts it `Unroutable` and drops it *before* the buffer — while `handle` has
/// already recorded that data came off the stream, rightly, since the server is
/// not refusing us. A session ending on one is `delivered` with
/// `resume_from() == None`, which is the premise of the listener's
/// keep-the-mark rule.
///
/// ⚠️ **Against a conformant server this shape does not arrive**, and the
/// claim that it did was wrong twice over before review caught it on
/// 16 September 2026. `build_request` names every transaction filter
/// `Protocol::as_str()` and `protocol_of` parses it back with `FromStr` — an
/// exact inverse, guarded by `every_protocol_name_round_trips_through_the_filter`
/// — so an update that arrives at all carries a name that parses. An
/// address-lookup table changes *which* transactions match, never whether the
/// match is named. What `Unroutable` counts is a filter name our request never
/// emits, which is what `on_transaction`'s own comment says: the request and
/// this reader disagree.
///
/// So this fixture builds a **non-conformant** update on purpose. It is still
/// the right one: the pair it produces is what the listener must survive, the
/// counter exists because the divergence is possible, and the rule under test —
/// an absent mark is not a mark at zero — is cheaper to hold than to
/// re-establish. What it is not is evidence of frequency.
pub(super) fn unroutable_transaction(slot: u64) -> SubscribeUpdate {
    transaction(slot, &[UNROUTABLE_FILTER])
}

/// A transaction update wrapped in the filters it matched.
///
/// ⚠️ The filter names are the routing — see `subscription`. Passing anything
/// but [`PROTOCOL`]`.as_str()` makes the update **unroutable**, which
/// `on_transaction` drops before it ever reaches the buffer. That is a case
/// worth testing on purpose and a silent no-op when it happens by accident.
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

/// A server keep-alive: the one message that is **not** data.
///
/// It matches no filter, which is also true on the wire — a ping is not a
/// subscription result. What the whole retry budget turns on is that
/// `StreamSession::received_data` stays false when one of these goes past.
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
