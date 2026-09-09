//! What the gRPC path hands to the rest of the pipeline.

use yog_core::{application::extraction::OnChainTransaction, domain::Protocol};

/// A transaction, translated and timestamped, with the protocol it belongs to.
///
/// # Why this is not a `QualifiedSignature`
///
/// Because the two paths hand over work at different points. `logsSubscribe`
/// notifies and the pipeline then *asks* — so what travels there is a signature,
/// and `TransactionProcessor` starts with a `getTransaction`. Yellowstone
/// **delivers**: the transaction is already in hand, with its `index` in the
/// block, and there is nothing left to fetch. So this path enters the pipeline
/// lower down, and what travels is the transaction itself.
///
/// The protocol rides along rather than being re-derived: the subscription's
/// filters are named after protocols, so the update said which one it matched.
/// See `subscription::protocol_of`.
#[derive(Debug)]
pub(crate) struct IngestedTransaction {
    pub(crate) protocol: Protocol,
    pub(crate) transaction: OnChainTransaction,
}
