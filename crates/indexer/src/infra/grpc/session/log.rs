//! The lines a stream session writes, one function each — same shape as the
//! listener's `log`.

use tracing::{debug, warn};
use yog_core::CoreError;

/// A transaction matched no protocol filter and was dropped.
pub(super) fn unroutable(slot: u64) {
    warn!(
        slot,
        "transaction update matched no protocol filter — dropping it"
    );
}

/// A block-meta came without a usable block time; its slot is given up.
pub(super) fn no_block_time(slot: u64) {
    warn!(
        slot,
        "block-meta carried no usable block time — giving up on the slot"
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
