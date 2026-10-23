//! The lines a stream session writes, one function each.

use tracing::{debug, info, warn};
use yog_core::CoreError;

use crate::infra::grpc::slot_timestamp_buffer::Bank;

/// A transaction matched no protocol filter and was dropped.
pub(super) fn unroutable(slot: u64) {
    warn!(
        slot,
        "transaction update matched no protocol filter — dropping it"
    );
}

/// A block-meta came without a usable block time; its bank is given up.
pub(super) fn no_block_time(slot: u64, bank: Bank) {
    warn!(
        slot,
        ?bank,
        "block-meta carried no usable block time — giving up on its bank"
    );
}

/// The session's first message naming no bank: from here on, its slots are paired by slot alone.
pub(super) fn no_bank_id(slot: u64) {
    info!(
        slot,
        "the server names no bank — each slot is paired by slot alone, \
         so two banks of one slot cannot be told apart"
    );
}

/// A transaction could not be translated and was dropped.
pub(super) fn untranslatable(slot: u64, error: &CoreError) {
    warn!(slot, %error, "could not translate a transaction");
}

/// The consumer is full: the stream waits for it.
pub(super) fn downstream_full() {
    debug!("downstream is full — slowing the stream to its speed");
}

/// A server ping could not be answered.
pub(super) fn ping_answer_unsent(reason: &str) {
    warn!(
        reason,
        "could not answer a server ping — the server may close the stream"
    );
}
