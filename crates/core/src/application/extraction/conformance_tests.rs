//! The arbiter's own guards.
//!
//! These do **not** prove the expectation is what mainnet sent — only the
//! JSON-RPC adapter's conformance test, which reaches this module from the
//! verbatim fixture, can do that. What they catch is the failure mode a
//! hand-transcribed constant actually has: a digit dropped or a line lost while
//! copying 392 hex characters twice.

use super::*;
use crate::application::extraction::anchor_event::{DISCRIMINATOR_LEN, EVENT_IX_TAG};
use crate::application::extraction::meteora::damm_v2::events::discriminator_swap2;

#[test]
fn both_reference_payloads_are_anchor_event_emissions() {
    // A dropped digit shifts every byte after it; a mistyped one does not. So
    // check both ends of the framing — Anchor's tag, then the event
    // discriminator that follows it — plus the length. Cheaper than re-reading
    // 392 characters, and it fails loudly.
    let swap2 = discriminator_swap2();

    for (index, payload) in reference_transaction()
        .inner_instructions
        .iter()
        .enumerate()
    {
        assert_eq!(
            &payload.data[..EVENT_IX_TAG.len()],
            &EVENT_IX_TAG,
            "payload {index} does not start with the Anchor event_cpi tag — \
             a hex digit was likely lost in transcription"
        );
        assert_eq!(
            &payload.data[EVENT_IX_TAG.len()..EVENT_IX_TAG.len() + DISCRIMINATOR_LEN],
            &swap2,
            "payload {index} does not carry the EvtSwap2 discriminator — the \
             reference transaction is two swaps and nothing else"
        );
        assert_eq!(
            payload.data.len(),
            196,
            "payload {index} is not the length the fixture carries"
        );
    }
}

#[test]
fn the_two_reference_payloads_are_distinct() {
    // The whole point of the reference is to witness *order*. Two identical
    // payloads would make any permutation invisible, and this assertion is what
    // stops a future edit from quietly choosing such a transaction.
    let tx = reference_transaction();
    assert_ne!(
        tx.inner_instructions[0].data, tx.inner_instructions[1].data,
        "the reference must carry two different payloads, or reordering them \
         could not be detected"
    );
}

#[test]
fn assert_matches_reference_accepts_the_reference() {
    // The control. Without it, the two tests below could pass because the
    // comparison never looked at anything.
    assert_matches_reference(&reference_transaction());
}

#[test]
#[should_panic(expected = "payload 0: bytes differ")]
fn a_permuted_transaction_is_rejected() {
    let mut tx = reference_transaction();
    tx.inner_instructions.swap(0, 1);
    assert_matches_reference(&tx);
}

#[test]
#[should_panic(expected = "payload count")]
fn a_dropped_payload_is_rejected() {
    let mut tx = reference_transaction();
    tx.inner_instructions.pop();
    assert_matches_reference(&tx);
}

#[test]
fn decode_hex_ignores_the_layout_whitespace() {
    assert_eq!(decode_hex("00ff"), vec![0x00, 0xff]);
    assert_eq!(decode_hex("  00\n  ff  "), vec![0x00, 0xff]);
}
