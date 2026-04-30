//! Generic Anchor event decoding for `event_cpi`-style events.
//!
//! Anchor offers two ways for a program to emit events:
//! 1. **`emit!`** — writes the event payload as a `Program data: <base64>`
//!    log line.
//! 2. **`emit_cpi!`** — performs a self-CPI to the program with the event
//!    payload as instruction data, recorded under `inner_instructions` in
//!    the transaction metadata.
//!
//! Meteora's cp-amm program uses the `emit_cpi!` form. This module supports
//! that form exclusively. The legacy log-based form is intentionally not
//! covered — adding it later is straightforward if a future protocol needs it.
//!
//! ## Wire format of an Anchor `event_cpi` payload
//!
//! ```text
//! [8 bytes EVENT_IX_TAG][8 bytes event discriminator][borsh payload]
//! ```
//!
//! - `EVENT_IX_TAG` is a constant defined by Anchor itself
//!   (`sha256("anchor:event")[..8]`). It identifies the inner instruction
//!   as an Anchor self-CPI event, regardless of which program emitted it.
//! - The event discriminator is `sha256("event:<EventName>")[..8]`.
//! - The borsh payload mirrors the event struct's field layout.
//!
//! ## Where event payloads come from
//!
//! When a program uses `emit_cpi!`, the runtime records a CPI to the
//! program itself with:
//! - `program_id == <emitting program>`
//! - `accounts == [event_authority]` (a single account, the program's PDA
//!   acting as event signer)
//! - `data == [tag][discriminator][payload]` encoded as base58 in
//!   `inner_instructions[].instructions[].data`
//!
//! [`extract_anchor_event_cpis`] isolates these instructions; the caller
//! then runs each through [`decode_anchor_event_cpi`] to obtain the
//! discriminator and payload, and finally borsh-deserializes the payload
//! into a known struct.

use bs58::decode as bs58_decode;
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedTransactionWithStatusMeta, UiInstruction,
    UiParsedInstruction,
};

use crate::error::AnchorDecodeError;

/// Length of an Anchor event discriminator, in bytes.
pub const DISCRIMINATOR_LEN: usize = 8;

/// Length of the Anchor `event_cpi` self-CPI tag, in bytes.
pub const EVENT_IX_TAG_LEN: usize = 8;

/// The constant tag Anchor prepends to every `event_cpi` payload.
///
/// Equal to `sha256("anchor:event")[..8]`, in the order produced by the
/// hash function — same order as on-chain bytes.
///
/// This tag is identical for every Anchor program — it's part of the
/// Anchor framework, not specific to any individual program.
pub const EVENT_IX_TAG: [u8; EVENT_IX_TAG_LEN] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Decode the raw bytes of an Anchor `event_cpi` instruction.
///
/// Validates that the leading 8 bytes match [`EVENT_IX_TAG`], then splits
/// the remainder into the event discriminator (next 8 bytes) and the borsh
/// payload (everything after).
///
/// Returns `Err(NotAnAnchorEvent)` if the tag does not match — the caller
/// can use this to filter out cp-amm inner instructions that aren't events
/// (regular CPI calls performed by the program for other reasons).
pub fn decode_anchor_event_cpi(
    data: &[u8],
) -> Result<([u8; DISCRIMINATOR_LEN], Vec<u8>), AnchorDecodeError> {
    let min_len = EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN;
    if data.len() < min_len {
        return Err(AnchorDecodeError::PayloadTooShort {
            min: min_len,
            got: data.len(),
        });
    }

    if data[..EVENT_IX_TAG_LEN] != EVENT_IX_TAG {
        return Err(AnchorDecodeError::NotAnAnchorEvent);
    }

    let mut disc = [0u8; DISCRIMINATOR_LEN];
    disc.copy_from_slice(&data[EVENT_IX_TAG_LEN..EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN]);

    let payload = data[EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN..].to_vec();

    Ok((disc, payload))
}

// ---------------------------------------------------------------------------
// Extraction from a Solana transaction
// ---------------------------------------------------------------------------

/// Extract every inner instruction in `tx` that targets `target_program_id`
/// and could be an Anchor `event_cpi` event emission.
///
/// Each returned entry is the **decoded base58 bytes** of an inner
/// instruction's `data` field — ready to be passed to
/// [`decode_anchor_event_cpi`].
///
/// Filtering is intentionally permissive: we keep every inner instruction
/// addressed to the target program, regardless of how many accounts it
/// references. Discriminating "is this really an event" is the job of
/// [`decode_anchor_event_cpi`], which checks the [`EVENT_IX_TAG`]
/// prefix. This keeps the extractor stable across Anchor framework
/// upgrades that might change incidental conventions like account counts.
///
/// Instructions encoded as `Parsed` (rather than `PartiallyDecoded`) are
/// skipped, because parsed instructions are produced for instructions the
/// RPC's transaction parser recognizes (SPL Token, etc.) — Anchor self-CPI
/// instructions never fall into that category.
pub fn extract_anchor_event_cpis(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    target_program_id: &str,
) -> Vec<Vec<u8>> {
    let Some(meta) = tx.transaction.meta.as_ref() else {
        return Vec::new();
    };

    let OptionSerializer::Some(inner_groups) = &meta.inner_instructions else {
        return Vec::new();
    };

    let mut out = Vec::new();

    for group in inner_groups {
        for ix in &group.instructions {
            if let Some(bytes) = try_extract_self_cpi_data(ix, target_program_id) {
                out.push(bytes);
            }
        }
    }

    out
}

/// Try to extract the raw bytes of an inner instruction whose `programId`
/// matches `target_program_id`. Returns `None` if the instruction targets
/// a different program, has no `data` field, or fails base58 decoding.
fn try_extract_self_cpi_data(ix: &UiInstruction, target_program_id: &str) -> Option<Vec<u8>> {
    let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p)) = ix else {
        return None;
    };

    if p.program_id != target_program_id {
        return None;
    }

    bs58_decode(&p.data).into_vec().ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal valid event_cpi byte sequence.
    fn build_event_bytes(disc: [u8; DISCRIMINATOR_LEN], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(EVENT_IX_TAG_LEN + DISCRIMINATOR_LEN + payload.len());
        out.extend_from_slice(&EVENT_IX_TAG);
        out.extend_from_slice(&disc);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn decodes_well_formed_event() {
        let disc = [9u8, 8, 7, 6, 5, 4, 3, 2];
        let payload = [42u8, 43, 44];
        let bytes = build_event_bytes(disc, &payload);

        let (got_disc, got_payload) = decode_anchor_event_cpi(&bytes).unwrap();
        assert_eq!(got_disc, disc);
        assert_eq!(got_payload, payload);
    }

    #[test]
    fn decodes_event_with_empty_payload() {
        let disc = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let bytes = build_event_bytes(disc, &[]);

        let (got_disc, got_payload) = decode_anchor_event_cpi(&bytes).unwrap();
        assert_eq!(got_disc, disc);
        assert!(got_payload.is_empty());
    }

    #[test]
    fn rejects_payload_below_minimum_size() {
        // Just a few bytes — not even enough for the tag.
        let bytes = vec![1u8, 2, 3];
        let err = decode_anchor_event_cpi(&bytes).unwrap_err();
        assert!(matches!(
            err,
            AnchorDecodeError::PayloadTooShort { got: 3, .. }
        ));
    }

    #[test]
    fn rejects_payload_with_wrong_tag() {
        // 16 bytes of zeros — long enough, but tag is wrong.
        let bytes = vec![0u8; 16];
        let err = decode_anchor_event_cpi(&bytes).unwrap_err();
        assert!(matches!(err, AnchorDecodeError::NotAnAnchorEvent));
    }

    #[test]
    fn tag_constant_matches_documented_value() {
        // EVENT_IX_TAG_LE must equal the bytes 1d 9a cb 51 2e a5 45 e4 —
        // this is sha256("anchor:event")[..8] in little-endian.
        // If this assertion ever fails, every Anchor program's events
        // would be unparseable; this guards against accidental edits.
        assert_eq!(
            EVENT_IX_TAG,
            [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d]
        );
    }
}
