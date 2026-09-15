//! Refuse a transaction the source did not describe — in the words both
//! adapters use.
//!
//! This is where `infra/rpc/transaction_adapter.rs` and
//! `infra/grpc/transaction_adapter.rs` come to say no, so that the two say it
//! identically. Each reads its own wire format and finds the same two gaps;
//! what they must not do is name them differently.
//!
//! # Why the wording is the point, and not an implementation detail
//!
//! `meta` and `meta.inner_instructions` can each arrive saying "the source did
//! not capture this", which is **not** "there was none" — the distinction both
//! adapters exist to keep.
//!
//! What an operator does about it differs by *which* of the two it was: a
//! provider that dropped `meta` wholesale and one that stopped recording inner
//! instructions are two different fixes. And the only place the two are told
//! apart is a log line — `session.rs`'s `warn!(%error, …)` on the gRPC path,
//! `fetch_worker.rs`'s `error!` on the other. The counters cannot help:
//! `drop_reason` folds both into `reason="missing_field"` and `FetchWorker`
//! folds every adaptation failure into `reason="adapt"`, both on purpose, to
//! bound cardinality.
//!
//! So the wording *is* the operator-facing contract, and it was written four
//! times — twice per adapter — with nothing checking that the two agreed.
//! Rewording one side and not the other would have left both suites green while
//! the same gap printed two different strings depending on which source had
//! ingested the transaction, and an operator grepping for one of them would have
//! seen half their traffic. One definition removes the possibility rather than
//! guarding against it.
//!
//! # ⚠️ What one definition costs, and what pays it back
//!
//! Swapping the two **values** below moves every adapter and every test
//! together, so nothing downstream can notice — an exposure the four separate
//! literals did not have. `refusal_tests.rs` closes it by pinning the text of
//! both constants at this one site: a change-detector on purpose, because
//! rewording the contract an operator greps for is a decision, not the
//! by-product of a refactor.

use solana_signature::Signature;
use yog_core::CoreError;

/// The whole of `meta` is missing — on the JSON-RPC path an absent or `null`
/// key, on the gRPC path an absent `meta` message.
pub(crate) const META: &str = "meta (not captured by the source)";

/// `meta` is there but says nothing about inner instructions — `None` on the
/// JSON-RPC path (an absent key and an explicit `null` both land there),
/// `inner_instructions_none` on the gRPC path, a flag the proto carries
/// precisely so the two cannot be confused.
pub(crate) const INNER_INSTRUCTIONS: &str = "meta.inner_instructions (not captured by the source)";

/// Refuse a transaction whose source did not capture `field`.
///
/// `field` is one of the two constants above, and taking `&'static str` rather
/// than an enum is deliberate: the caller reads
/// `refusal::refuse(META, signature)`, which says at the call site both what it
/// is doing and which of the two gaps it is answering — the two things a reader
/// needs there, and what the mutation check exercises.
///
/// The `refusal::refuse` repetition is kept rather than avoided. Importing
/// `refuse` bare would read a shade better and drop the one word that tells the
/// next author this text is shared: the module has to stay visible at the call
/// site, or the literal comes back.
pub(crate) fn refuse(field: &'static str, signature: &Signature) -> CoreError {
    CoreError::MissingField {
        signature: signature.to_string(),
        field: field.to_string(),
    }
}

#[cfg(test)]
#[path = "refusal_tests.rs"]
mod tests;
