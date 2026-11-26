//! Decoding of on-chain **pool accounts** into the properties events never carry.
//!
//! ⚠️ Each decoder checks the owner (by dispatch) **and** the discriminator: any
//! 32 bytes parse as a `Pubkey`, so a foreign account decodes without an error.

mod meteora;
mod pool_account_rejection;

use solana_pubkey::Pubkey;

use crate::domain::{DecodedPoolAccount, Protocol};

pub use pool_account_rejection::PoolAccountRejection;

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
