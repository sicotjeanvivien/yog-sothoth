//! What every source adapter must produce, and the reference to prove it on.
//!
//! [`OnChainTransaction`] carries a contract its own crate cannot enforce: the
//! order of `inner_instructions` decides the persisted `event_index`, and the
//! adapters that fill it live outside `core` — one per source. Two adapters,
//! two independent test suites and a single contract is the configuration where
//! both drift and nothing turns red. This module is the shared arbiter.
//!
//! # How it is meant to be used
//!
//! Each adapter feeds its own rendering of the reference transaction through
//! itself and calls [`assert_matches_reference`]. The JSON-RPC adapter starts
//! from the mainnet fixture; a protobuf adapter will start from a hand-built
//! `SubscribeUpdate`. Same expectation, one place.
//!
//! # Why a hand-written expectation is sound here, and only here
//!
//! Building an `OnChainTransaction` by hand inside a test is normally the wrong
//! move: the test and the code then agree on the author's understanding of the
//! format instead of confronting it with a real transaction. That objection is
//! answered by **one** test — the JSON-RPC adapter's conformance test, which
//! reaches this expectation from the verbatim mainnet response in
//! `core/tests/fixtures/damm_v2/swap_double.json`. That test is what pins the
//! bytes below to reality; every other consumer trusts them because of it. Delete
//! it and this module becomes exactly the trap it exists to avoid.
//!
//! # Why this transaction
//!
//! `2qJrr…` routes two swaps through the same pool under one signature and one
//! `blockTime` — it is already the repository's reference for the defect that
//! `event_index` was introduced to fix. Its two payloads sit in **different**
//! inner-instruction groups (5 and 8), so an adapter that flattens groups in the
//! wrong order produces a different result here. A transaction whose payloads
//! shared a group could not witness the order contract at all.

use std::str::FromStr;

use chrono::DateTime;
use solana_signature::Signature;

use crate::application::extraction::{InnerInstructionPayload, OnChainTransaction};
use crate::domain::{Protocol, TransactionPosition};

/// Signature of the reference transaction — its id, and what it is fetched by.
const REFERENCE_SIGNATURE: &str =
    "2qJrrEVDC3DipMWmd8WgfKRaXBu9RFZLi8udbxW6NKPQ4kfU1fz8pZ6zeepKRG2EoMQ2rY3BhQ96yHSGMqxoh6NN";

/// Slot the reference transaction landed in.
const REFERENCE_SLOT: u64 = 415_296_180;

/// Its `blockTime`, in seconds since the epoch.
const REFERENCE_BLOCK_TIME: i64 = 1_777_013_942;

/// First cp-amm payload — the A→B leg, emitted from inner-instruction group 5.
///
/// Hexadecimal rather than base58 on purpose: base58 would put `bs58` back into
/// `core`, which is one of the two dependencies this arrangement exists to
/// remove. Hex also diffs and reads, which a binary blob does not.
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
            // Absent from this response. Optional in the API, and the provider
            // in use never returns it — see `EventPosition`.
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

/// Assert that `actual` is the reference transaction, naming what diverged.
///
/// Panics with the offending payload index or position field rather than two
/// several-hundred-byte `Debug` blobs, because the failure this guards against
/// is a *reordering* — and two orderings of the same bytes look identical until
/// something points at the index that moved.
pub fn assert_matches_reference(actual: &OnChainTransaction) {
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
        actual.position.transaction_index, expected.position.transaction_index,
        "transaction_index"
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

/// Decode an even-length hex string, ignoring the whitespace that lets the
/// constants above be read 32 bytes to a line.
///
/// Ten lines rather than a dependency: `core` is shedding two here, and adding
/// one back for test data would undo the point.
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
