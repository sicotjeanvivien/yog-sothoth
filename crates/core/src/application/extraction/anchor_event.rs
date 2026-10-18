//! Generic decoding of Anchor `emit_cpi!` events (not `emit!` log lines): a
//! self-CPI whose data is `[EVENT_IX_TAG][discriminator][borsh payload]`.

use solana_pubkey::Pubkey;

use crate::application::extraction::OnChainTransaction;
use crate::error::AnchorDecodeError;

/// Length of an Anchor event discriminator, in bytes.
pub const DISCRIMINATOR_LEN: usize = 8;

/// Length of the Anchor `event_cpi` self-CPI tag, in bytes.
pub const EVENT_IX_TAG_LEN: usize = 8;

/// `sha256("anchor:event")[..8]`, the same for every Anchor program.
pub const EVENT_IX_TAG: [u8; EVENT_IX_TAG_LEN] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Split an Anchor `event_cpi` payload into its discriminator and borsh body;
/// `NotAnAnchorEvent` marks an ordinary instruction to the program.
pub(crate) fn decode_anchor_event_cpi(
    data: &[u8],
) -> Result<([u8; DISCRIMINATOR_LEN], Vec<u8>), AnchorDecodeError> {
    // Shorter than an instruction discriminator: not even an ordinary instruction.
    if data.len() < DISCRIMINATOR_LEN {
        return Err(AnchorDecodeError::PayloadTooShort {
            min: DISCRIMINATOR_LEN,
            got: data.len(),
        });
    }

    // ⚠️ Tag before length: an argument-less instruction (8 bytes) would otherwise be a failure.
    if !data.starts_with(&EVENT_IX_TAG) {
        return Err(AnchorDecodeError::NotAnAnchorEvent);
    }

    let min_len = EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN;
    if data.len() < min_len {
        return Err(AnchorDecodeError::PayloadTooShort {
            min: min_len,
            got: data.len(),
        });
    }

    let mut disc = [0u8; DISCRIMINATOR_LEN];
    disc.copy_from_slice(&data[EVENT_IX_TAG_LEN..EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN]);

    let payload = data[EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN..].to_vec();

    Ok((disc, payload))
}

// ---------------------------------------------------------------------------
// Extraction from a Solana transaction
// ---------------------------------------------------------------------------

/// Every inner-instruction payload of `tx` addressed to `target_program_id`.
/// ⚠️ Only ever widen this filter: the output position is the persisted
/// `event_index` ([`InnerInstructionPayload`](crate::application::extraction::InnerInstructionPayload)).
pub(crate) fn extract_anchor_event_cpis<'a>(
    tx: &'a OnChainTransaction,
    target_program_id: &Pubkey,
) -> Vec<&'a [u8]> {
    tx.inner_instructions
        .iter()
        .filter(|ix| ix.program_id == *target_program_id)
        .map(|ix| ix.data.as_slice())
        .collect()
}

#[cfg(test)]
#[path = "anchor_event_tests.rs"]
mod tests;
