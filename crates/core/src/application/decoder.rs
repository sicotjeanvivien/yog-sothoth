//! Decoding of on-chain **pool accounts** into domain properties.
//!
//! The counterpart of [`super::extraction`]: where that module turns a
//! transaction's event bytes into domain events, this one turns an account's
//! raw bytes into the pool properties that events never carry — the token
//! mints, the base fee, the fee-split percents.
//!
//! Both are protocol-specific byte layouts, both are pure, and both belong in
//! `core` for the same reason: they are the project's knowledge of what the
//! chain means, and the crates that do I/O should not have to own it.
//!
//! # Why this is not in `amm/`
//!
//! `amm/` is *math* — spot price, price impact, fee arithmetic. This is layout
//! decoding. (The fee-blob decoders in `amm::damm_v2` predate the distinction
//! and sit on the wrong side of it; moving them is tracked separately, since it
//! would churn the indexer for no functional gain.)
//!
//! # Dispatch
//!
//! An account's `owner` **is** its program id, so [`decode_pool_account`] routes
//! on it via [`Protocol::from_program_id`] — the caller does not have to know
//! which protocol it asked for, which is what lets one RPC client serve every
//! protocol.
//!
//! # The two guards, and why they are load-bearing
//!
//! Every decoder checks the owner (here, by dispatch) **and** the account's
//! Anchor discriminator. Neither is redundant.
//!
//! Reading a foreign account at cp-amm's offsets does not produce obvious
//! garbage: at byte 168 and 200 of a DLMM `LbPair` sit `reserve_x` and
//! `reserve_y` — valid, aligned `Pubkey`s. Without the guards, `Pubkey::try_from`
//! succeeds, no error is raised, and the pool's *vault* addresses are written
//! into its *mint* columns. Silently wrong data, not a crash.

mod meteora;

use solana_pubkey::Pubkey;
use thiserror::Error;

use crate::domain::{DecodedPoolAccount, Protocol};

/// Why an account was not decoded.
///
/// Deliberately **not** an `Option`. In this crate's only call path the worker
/// asks for accounts of pools *it queued itself*, from a queue scoped to one
/// protocol — so none of these outcomes is routine. Each one names a distinct
/// problem worth a distinct log line or metric, and collapsing them into a bare
/// `None` is how a silent failure hides: the pool never resolves, stays in the
/// queue, and is re-fetched every cycle forever with nothing to show for it.
///
/// Same discipline as [`super::extraction::ExtractionFailure`]: `core` does no
/// I/O, so it returns the structured reason and the caller logs and counts it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PoolAccountRejection {
    /// The account is owned by a program we do not index. In this path that is
    /// surprising, not benign: it means a row in `pools` points at an account
    /// that is not the protocol we recorded.
    #[error("account owned by an unindexed program: {program_id}")]
    UnknownProgram { program_id: Pubkey },

    /// The protocol is one we know, but its pool account has no decoder yet.
    /// A coverage gap — and, if it appears, a wiring bug: a queue exists for a
    /// protocol we cannot read.
    #[error("no pool-account decoder for {protocol}")]
    NoDecoder { protocol: Protocol },

    /// The account belongs to the right program but is not its pool account —
    /// its Anchor discriminator does not match. The address in `pools` is not
    /// what we think it is.
    #[error("{protocol}: not a pool account (unexpected discriminator)")]
    NotAPoolAccount { protocol: Protocol },

    /// The account is shorter than the layout requires. **The one to watch**:
    /// the most likely cause is the program shipping an ABI change, which is
    /// exactly the kind of drift that must not pass unnoticed.
    #[error("{protocol}: account too short — {len} bytes, layout needs {min}")]
    Truncated {
        protocol: Protocol,
        len: usize,
        min: usize,
    },
}

/// Decode a raw pool account into its protocol's properties.
///
/// `owner` selects the layout; `data` is the account's raw bytes, already
/// base64-decoded by the transport (base64 is the RPC's encoding, not the
/// chain's — `core` stays free of it).
///
/// On failure returns a typed [`PoolAccountRejection`] rather than a bare
/// absence — see that type for why the distinction is load-bearing here.
pub fn decode_pool_account(
    program_id: &Pubkey,
    data: &[u8],
) -> Result<DecodedPoolAccount, PoolAccountRejection> {
    let protocol =
        Protocol::from_program_id(program_id).ok_or(PoolAccountRejection::UnknownProgram {
            program_id: *program_id,
        })?;

    match protocol {
        Protocol::MeteoraDammV2 => meteora::damm_v2::decode_pool_account(data),
        Protocol::MeteoraDlmm => meteora::dlmm::decode_pool_account(data),
        // Recognized protocols with no pool-account decoder yet — reported as a
        // coverage gap, not as an unknown program.
        Protocol::MeteoraDammV1 => Err(PoolAccountRejection::NoDecoder { protocol }),
    }
}

#[cfg(test)]
#[path = "decoder_tests.rs"]
mod tests;
