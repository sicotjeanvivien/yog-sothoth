//! The refusal both transaction adapters give a transaction the source did not describe, worded
//! once: the log line is the only place an operator tells the two gaps apart.
//! ⚠️ `refusal_tests.rs` pins the text on purpose: rewording what an operator greps for is a decision.

use solana_signature::Signature;
use yog_core::CoreError;

/// A gap **both** adapters can meet; one only an adapter can raise stays inline there.
/// ⚠️ An enum, not a `&'static str`: a string would let the next shared gap return as two literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gap {
    /// The whole of `meta` is missing — on the JSON-RPC path an absent or
    /// `null` key, on the gRPC path an absent `meta` message.
    Meta,

    /// `meta` says nothing about inner instructions: `None` on the JSON-RPC path,
    /// `inner_instructions_none` on the gRPC path.
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
/// ⚠️ Called as `refusal::refuse`, not imported bare: the module name says the text is shared.
pub(crate) fn refuse(gap: Gap, signature: &Signature) -> CoreError {
    CoreError::MissingField {
        signature: signature.to_string(),
        field: gap.field().to_string(),
    }
}

#[cfg(test)]
#[path = "refusal_tests.rs"]
mod tests;
