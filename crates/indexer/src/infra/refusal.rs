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
//! seen half their traffic.
//!
//! One definition removes that possibility rather than guarding against it —
//! and [`Gap`] is what extends the removal to the gap this module does not know
//! about yet. A `&'static str` parameter would have fixed today's two labels
//! and left the next one free to arrive as a literal at two call sites; a
//! variant cannot be added anywhere but here.
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

/// A gap **both** adapters can meet, and that is the whole admission rule for
/// this module.
///
/// The four `MissingField`s left inline in the two adapters are not oversights:
/// `transaction` and `transaction.message` exist only on the protobuf side,
/// `signatures` and `blockTime` only on the JSON-RPC envelope. One adapter can
/// word those alone, because only it can raise them. A variant here means the
/// opposite — two adapters must answer with one voice — so adding one is a
/// claim about both wire formats, to be made at this site and nowhere else.
///
/// ⚠️ **This enum is the point, not decoration.** A `&'static str` parameter
/// would let the next shared gap be written as a literal at two call sites,
/// compile clean, and rebuild in silence the four-literal drift this module was
/// made to remove — and more quietly than the first time, because the module's
/// existence reads as the problem being solved. Raised in review, 15 September
/// 2026.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gap {
    /// The whole of `meta` is missing — on the JSON-RPC path an absent or
    /// `null` key, on the gRPC path an absent `meta` message.
    Meta,

    /// `meta` is there but says nothing about inner instructions — `None` on
    /// the JSON-RPC path (an absent key and an explicit `null` both land
    /// there), `inner_instructions_none` on the gRPC path, a flag the proto
    /// carries precisely so the two cannot be confused.
    InnerInstructions,
}

impl Gap {
    /// The words an operator reads, and the reason this module exists.
    pub(crate) const fn field(self) -> &'static str {
        match self {
            Self::Meta => "meta (not captured by the source)",
            Self::InnerInstructions => "meta.inner_instructions (not captured by the source)",
        }
    }
}

/// Refuse a transaction whose source did not describe `gap`.
///
/// The caller reads `refusal::refuse(Gap::Meta, signature)`, which says at the
/// call site both what it is doing and which of the gaps it is answering — the
/// two things a reader needs there, and what the mutation check exercises.
///
/// The [`refusal::refuse`] repetition is kept rather than avoided. Importing
/// `refuse` bare would read a shade better and drop the one word that tells the
/// next author this text is shared: the module has to stay visible at the call
/// site, or the literal comes back.
///
/// [`refusal::refuse`]: crate::infra::refusal::refuse
pub(crate) fn refuse(gap: Gap, signature: &Signature) -> CoreError {
    CoreError::MissingField {
        signature: signature.to_string(),
        field: gap.field().to_string(),
    }
}

#[cfg(test)]
#[path = "refusal_tests.rs"]
mod tests;
