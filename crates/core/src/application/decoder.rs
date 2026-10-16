//! Decoding of on-chain **pool accounts** into the properties events never carry.
//!
//! ⚠️ Each decoder checks the owner (by dispatch) **and** the discriminator: read
//! at cp-amm's offsets, a DLMM `LbPair` yields valid vault `Pubkey`s as mints.

mod meteora;

use solana_pubkey::Pubkey;
use thiserror::Error;

use crate::domain::{DecodedPoolAccount, Protocol};

/// Why an account was not decoded.
/// ⚠️ Not an `Option`: a pool that never resolves is re-fetched every cycle, so
/// each reason must reach its own log line and metric.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PoolAccountRejection {
    /// Owned by a program we do not index: a `pools` row points at the wrong account.
    #[error("account owned by an unindexed program: {program_id}")]
    UnknownProgram { program_id: Pubkey },

    /// Right program, wrong account: the Anchor discriminator does not match.
    #[error("{protocol}: not a pool account (unexpected discriminator)")]
    NotAPoolAccount { protocol: Protocol },

    /// Shorter than the layout requires: most likely an ABI change of the program.
    #[error("{protocol}: account too short — {len} bytes, layout needs {min}")]
    Truncated {
        protocol: Protocol,
        len: usize,
        min: usize,
    },
}

/// Decode a raw pool account; `program_id` (its owner) selects the layout,
/// `data` is already base64-decoded by the transport.
pub fn decode_pool_account(
    program_id: &Pubkey,
    data: &[u8],
) -> Result<DecodedPoolAccount, PoolAccountRejection> {
    let protocol =
        Protocol::from_program_id(program_id).ok_or(PoolAccountRejection::UnknownProgram {
            program_id: *program_id,
        })?;

    // ⚠️ No `_` arm: a new `Protocol` must stop compilation here and ask for a decoder.
    match protocol {
        Protocol::MeteoraDammV2 => meteora::damm_v2::decode_pool_account(data),
        Protocol::MeteoraDlmm => meteora::dlmm::decode_pool_account(data),
    }
}

#[cfg(test)]
#[path = "decoder_tests.rs"]
mod tests;
