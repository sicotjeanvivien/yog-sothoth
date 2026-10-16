//! The reference transaction every source adapter must produce, and its arbiter.
//!
//! ⚠️ Sound only because the JSON-RPC adapter's conformance test reaches it from
//! the verbatim mainnet fixture: delete that test and these bytes prove nothing.
//! Its two payloads sit in different inner-instruction groups, so order shows.

use std::str::FromStr;

use chrono::DateTime;
use solana_signature::Signature;

use crate::application::extraction::{InnerInstructionPayload, OnChainTransaction};
use crate::domain::{Protocol, TransactionPosition};

/// Signature of the reference transaction.
const REFERENCE_SIGNATURE: &str =
    "2qJrrEVDC3DipMWmd8WgfKRaXBu9RFZLi8udbxW6NKPQ4kfU1fz8pZ6zeepKRG2EoMQ2rY3BhQ96yHSGMqxoh6NN";

/// Slot the reference transaction landed in.
const REFERENCE_SLOT: u64 = 415_296_180;

/// Its `blockTime`, in seconds since the epoch.
const REFERENCE_BLOCK_TIME: i64 = 1_777_013_942;

/// First cp-amm payload, the A→B leg, from inner-instruction group 5.
/// ⚠️ Hex, not base58: base58 would bring `bs58` back into `core`.
const LEG_A_HEX: &str = "\
    e445a52e51cb9a1dbd4233a826507599cb422f17f0e7100ec0a157eb01fb720f\
    557b0027b7416c913255aec50a533489000100450b2730020000002d9b7dbea4\
    12000000450b273002000000450b273002000000000000000000000024e9b378\
    d412000029af5ead1288a9582e0000000000000061aebb620000000098ebae18\
    0000000000000000000000000000000000000000450b27300200000024e9b378\
    d412000024e9b378d4120000b614eb6900000000b3391535570000004b14b4e9\
    b7db0200";

/// Second cp-amm payload — the B→A leg, emitted from group 8.
const LEG_B_HEX: &str = "\
    e445a52e51cb9a1dbd4233a826507599cb422f17f0e7100ec0a157eb01fb720f\
    557b0027b7416c913255aec50a5334890101009ce4bb310c140000ffffac2901\
    000000009ce4bb310c1400006b8b59ae0b1400000000000000000000bb2a4b53\
    02000000a8ab817ce3acb29d2f000000000000008e7a1b6900000000a3de461a\
    00000000000000000000000000000000000000009ce4bb310c140000bb2a4b53\
    02000000bb2a4b5302000000b614eb6900000000f80ecae154000000b69f0d98\
    c3ef0200";

/// The reference transaction in neutral form — what **every** adapter must
/// produce from its own rendering of it.
pub fn reference_transaction() -> OnChainTransaction {
    let program_id = Protocol::MeteoraDammV2.program_id();

    OnChainTransaction {
        position: TransactionPosition {
            signature: Signature::from_str(REFERENCE_SIGNATURE)
                .expect("the reference signature is a constant and must parse"),
            timestamp: DateTime::from_timestamp(REFERENCE_BLOCK_TIME, 0)
                .expect("the reference block time is a constant and must convert"),
            slot: REFERENCE_SLOT,
            // ⚠️ Not part of the shared expectation: [`assert_matches_reference`]
            // takes it as an argument.
            transaction_index: None,
        },
        inner_instructions: vec![
            InnerInstructionPayload {
                program_id,
                data: decode_hex(LEG_A_HEX),
            },
            InnerInstructionPayload {
                program_id,
                data: decode_hex(LEG_B_HEX),
            },
        ],
    }
}

/// Assert that `actual` is the reference transaction, naming the payload or field
/// that diverged. `transaction_index` is an argument: JSON-RPC yields `None`,
/// gRPC `Some`, and each caller states what its source provides.
pub fn assert_matches_reference(
    actual: &OnChainTransaction,
    expected_transaction_index: Option<u32>,
) {
    let expected = reference_transaction();

    assert_eq!(
        actual.position.signature, expected.position.signature,
        "signature: the adapter did not read the transaction id"
    );
    assert_eq!(
        actual.position.timestamp, expected.position.timestamp,
        "timestamp: part of the unique key and the partitioning column"
    );
    assert_eq!(actual.position.slot, expected.position.slot, "slot");
    assert_eq!(
        actual.position.transaction_index, expected_transaction_index,
        "transaction_index: the caller states what its source provides — see \
         this function's docs for why this one field is not a constant"
    );

    assert_eq!(
        actual.inner_instructions.len(),
        expected.inner_instructions.len(),
        "payload count: a payload was dropped or invented, which shifts every \
         event_index after it"
    );

    for (index, (got, want)) in actual
        .inner_instructions
        .iter()
        .zip(expected.inner_instructions.iter())
        .enumerate()
    {
        assert_eq!(
            got.program_id, want.program_id,
            "payload {index}: emitting program"
        );
        assert_eq!(
            got.data, want.data,
            "payload {index}: bytes differ — if the payloads are the right ones \
             in the wrong order, this is the event_index contract breaking, not \
             a decoding bug"
        );
    }
}

/// Decode an even-length hex string, ignoring whitespace.
fn decode_hex(s: &str) -> Vec<u8> {
    let digits: Vec<u8> = s
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            other => panic!("not a hex digit: {}", other as char),
        })
        .collect();

    assert!(
        digits.len().is_multiple_of(2),
        "hex constant has an odd number of digits"
    );

    digits
        .chunks(2)
        .map(|pair| pair[0] << 4 | pair[1])
        .collect()
}

#[cfg(test)]
#[path = "conformance_tests.rs"]
mod tests;
