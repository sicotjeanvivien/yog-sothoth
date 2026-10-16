//! The arbiter's own guards: a digit or a line lost from the hand-transcribed hex.
//! Whether the bytes match mainnet is the JSON-RPC adapter's test, not these.

use super::*;
use crate::application::extraction::anchor_event::{DISCRIMINATOR_LEN, EVENT_IX_TAG};
use crate::application::extraction::meteora::damm_v2::events::discriminator_swap2;

#[test]
fn both_reference_payloads_are_anchor_event_emissions() {
    // A dropped digit shifts every byte after it: check the tag, the
    // discriminator and the length.
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
    // ⚠️ Two identical payloads would make any reordering invisible.
    let tx = reference_transaction();
    assert_ne!(
        tx.inner_instructions[0].data, tx.inner_instructions[1].data,
        "the reference must carry two different payloads, or reordering them \
         could not be detected"
    );
}

#[test]
fn assert_matches_reference_accepts_the_reference() {
    // The control: without it, the tests below could pass by looking at nothing.
    assert_matches_reference(&reference_transaction(), None);
}

#[test]
#[should_panic(expected = "payload 0: bytes differ")]
fn a_permuted_transaction_is_rejected() {
    let mut tx = reference_transaction();
    tx.inner_instructions.swap(0, 1);
    assert_matches_reference(&tx, None);
}

#[test]
#[should_panic(expected = "payload count")]
fn a_dropped_payload_is_rejected() {
    let mut tx = reference_transaction();
    tx.inner_instructions.pop();
    assert_matches_reference(&tx, None);
}

/// Mutation: `assert_matches_reference` ignoring its `transaction_index` argument.
#[test]
#[should_panic(expected = "transaction_index")]
fn a_transaction_index_the_caller_did_not_expect_is_rejected() {
    let mut tx = reference_transaction();
    tx.position.transaction_index = Some(7);
    assert_matches_reference(&tx, None);
}

/// Mutation: a `None` satisfying a caller that states `Some`.
#[test]
#[should_panic(expected = "transaction_index")]
fn a_missing_transaction_index_is_rejected_when_the_caller_expects_one() {
    assert_matches_reference(&reference_transaction(), Some(3));
}

#[test]
fn decode_hex_ignores_the_layout_whitespace() {
    assert_eq!(decode_hex("00ff"), vec![0x00, 0xff]);
    assert_eq!(decode_hex("  00\n  ff  "), vec![0x00, 0xff]);
}
