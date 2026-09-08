//! Tests for the Yellowstone protobuf adapter.
//!
//! ⚠️ Read this module's parent doc-comment before trusting anything green
//! here: the inputs are built by hand, so they carry the author's understanding
//! of the wire format and not an observation of it.
//!
//! # Why the reference rendering carries two inner instructions and not fourteen
//!
//! The mainnet fixture the shared arbiter is pinned to holds **14** inner
//! instructions across four groups; the JSON-RPC adapter keeps **2** of them,
//! dropping the twelve the RPC had already parsed into SPL Token shapes it
//! cannot represent as raw bytes. Protobuf has no such distinction — every
//! inner instruction arrives as `{program_id_index, accounts, data}` — so a
//! *faithful* rendering of that transaction would make this adapter produce all
//! fourteen.
//!
//! That is correct behaviour, not a bug: `InnerInstructionPayload`'s contract
//! says an adapter must be permissive, and that dropping payloads addressed to
//! **other** programs "costs nothing" because numbering happens after the filter
//! on the emitting program. The two adapters therefore legitimately produce
//! different vectors for the same transaction, and
//! `conformance::assert_matches_reference` compares whole vectors.
//!
//! So the rendering below is **deliberately reduced** to the two cp-amm
//! instructions — enough to witness what the arbiter is for (the order that
//! becomes `event_index`), and honest about being a reduction rather than a
//! transcription. What the reduction cannot show — that non-cp-amm instructions
//! are kept rather than filtered — is covered on its own by
//! [`every_inner_instruction_is_kept_whatever_its_program`].

use super::*;
use yellowstone_grpc_proto::prelude::{
    InnerInstructions, Message, SubscribeUpdateTransactionInfo, Transaction, TransactionStatusMeta,
};
use yog_core::application::extraction::conformance::{
    assert_matches_reference, reference_transaction,
};
use yog_core::domain::Protocol;

/// Signature of the reference transaction, base58 as an explorer shows it.
const REFERENCE_SIGNATURE: &str =
    "2qJrrEVDC3DipMWmd8WgfKRaXBu9RFZLi8udbxW6NKPQ4kfU1fz8pZ6zeepKRG2EoMQ2rY3BhQ96yHSGMqxoh6NN";

const REFERENCE_SLOT: u64 = 415_296_180;

/// The position in the slot a provider would report. Absent from the JSON-RPC
/// response, which is the whole reason this source exists — so any value does
/// here, as long as the test states it rather than reading it back.
const REFERENCE_INDEX: u64 = 42;

/// The timestamp the arbiter expects, since the adapter cannot read one and the
/// caller has to supply it. That the adapter really uses what it is handed —
/// rather than reaching for a constant — is the separate business of
/// [`the_timestamp_is_the_one_supplied`], which passes a different instant.
fn reference_timestamp() -> DateTime<Utc> {
    reference_transaction().position.timestamp
}

fn signature_bytes() -> Vec<u8> {
    bs58::decode(REFERENCE_SIGNATURE)
        .into_vec()
        .expect("the reference signature is a constant and must decode")
}

/// A pubkey that is not any real program, for the account slots the tests do
/// not care about.
fn filler_key(seed: u8) -> Vec<u8> {
    vec![seed; 32]
}

/// Build an update whose account keys are exactly `static_keys`, with the given
/// inner-instruction groups and no loaded addresses.
fn update_with(
    static_keys: Vec<Vec<u8>>,
    groups: Vec<InnerInstructions>,
) -> SubscribeUpdateTransaction {
    update_with_loaded(static_keys, Vec::new(), Vec::new(), groups)
}

/// The same, with the two loaded-address segments spelled out.
fn update_with_loaded(
    static_keys: Vec<Vec<u8>>,
    loaded_writable: Vec<Vec<u8>>,
    loaded_readonly: Vec<Vec<u8>>,
    groups: Vec<InnerInstructions>,
) -> SubscribeUpdateTransaction {
    SubscribeUpdateTransaction {
        slot: REFERENCE_SLOT,
        transaction: Some(SubscribeUpdateTransactionInfo {
            signature: signature_bytes(),
            is_vote: false,
            index: REFERENCE_INDEX,
            transaction: Some(Transaction {
                signatures: vec![signature_bytes()],
                message: Some(Message {
                    account_keys: static_keys,
                    ..Default::default()
                }),
            }),
            meta: Some(TransactionStatusMeta {
                inner_instructions: groups,
                loaded_writable_addresses: loaded_writable,
                loaded_readonly_addresses: loaded_readonly,
                ..Default::default()
            }),
        }),
    }
}

fn group(index: u32, instructions: Vec<InnerInstruction>) -> InnerInstructions {
    InnerInstructions {
        index,
        instructions,
    }
}

fn instruction(program_id_index: u32, data: Vec<u8>) -> InnerInstruction {
    InnerInstruction {
        program_id_index,
        accounts: Vec::new(),
        data,
        stack_height: None,
    }
}

/// The payloads the arbiter expects, taken from the arbiter itself rather than
/// re-transcribed: 392 hex characters copied a third time is 392 chances to
/// drop a digit, and the arbiter already guards its own transcription.
fn reference_payloads() -> (Vec<u8>, Vec<u8>) {
    let tx = reference_transaction();
    (
        tx.inner_instructions[0].data.clone(),
        tx.inner_instructions[1].data.clone(),
    )
}

// ── the shared contract ─────────────────────────────────────────────

/// The arbiter both adapters answer to. The two payloads sit in groups 5 and 8
/// as they do on chain, so a flattening that ignored group order would put them
/// the wrong way round and fail here — which is what the arbiter exists for.
#[test]
fn the_reference_transaction_is_what_this_adapter_produces() {
    let (leg_a, leg_b) = reference_payloads();
    let program = Protocol::MeteoraDammV2.program_id();

    let update = update_with(
        vec![filler_key(1), program.to_bytes().to_vec()],
        vec![
            group(5, vec![instruction(1, leg_a)]),
            group(8, vec![instruction(1, leg_b)]),
        ],
    );

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");

    assert_matches_reference(&actual, Some(REFERENCE_INDEX as u32));
}

/// The order contract, seen from the failure side: a source that hands its
/// groups back in descending order must still produce ascending payloads.
/// Providers are under no obligation to sort, and `event_index` is a stored key.
#[test]
fn groups_are_ordered_by_their_outer_instruction_not_by_arrival() {
    let (leg_a, leg_b) = reference_payloads();
    let program = Protocol::MeteoraDammV2.program_id();

    let update = update_with(
        vec![filler_key(1), program.to_bytes().to_vec()],
        vec![
            group(8, vec![instruction(1, leg_b)]),
            group(5, vec![instruction(1, leg_a)]),
        ],
    );

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");

    assert_matches_reference(&actual, Some(REFERENCE_INDEX as u32));
}

/// The timestamp is the one handed in, never one read off the message — the
/// message carries none, and inventing one would put a wrong value in the
/// partitioning column.
#[test]
fn the_timestamp_is_the_one_supplied() {
    let program = Protocol::MeteoraDammV2.program_id();
    let update = update_with(vec![program.to_bytes().to_vec()], Vec::new());

    let supplied = DateTime::from_timestamp(1_700_000_000, 0).expect("a valid instant");
    let actual = from_grpc(&update, supplied).expect("a well-formed update");

    assert_eq!(actual.position.timestamp, supplied);
    assert_ne!(
        actual.position.timestamp,
        reference_transaction().position.timestamp,
        "the adapter must not be reading a timestamp from anywhere"
    );
}

// ── resolving program_id_index ──────────────────────────────────────

/// ⚠️ **The test the reference transaction cannot be.** No transaction fixture
/// in this repository carries a `loadedAddresses` — 25 of the 92 use address
/// lookup tables, but the captured responses resolve nothing — so the
/// conformance test above passes even against an implementation that ignores
/// `loaded_*` entirely.
///
/// The program sits in `loaded_readonly_addresses`, behind a **non-empty**
/// `loaded_writable_addresses` — the only arrangement that tells the right order
/// from both wrong ones: reading readonly before writable resolves to the wrong
/// key, and skipping the loaded segments resolves to nothing.
#[test]
fn a_program_in_the_loaded_addresses_is_resolved_through_all_three_segments() {
    let (leg_a, _) = reference_payloads();
    let program = Protocol::MeteoraDammV2.program_id();

    // static: 2 keys (indices 0-1), writable: 2 (2-3), readonly: 2 (4-5).
    let update = update_with_loaded(
        vec![filler_key(1), filler_key(2)],
        vec![filler_key(3), filler_key(4)],
        vec![filler_key(5), program.to_bytes().to_vec()],
        vec![group(0, vec![instruction(5, leg_a)])],
    );

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");

    assert_eq!(actual.inner_instructions.len(), 1);
    assert_eq!(
        actual.inner_instructions[0].program_id, program,
        "index 5 must land on the second readonly key — static(2) + writable(2) \
         consumed first"
    );
}

/// An index past the end resolves to nothing, and that is an error rather than
/// an absent payload: dropping one silently renumbers every event after it.
#[test]
fn an_index_beyond_the_account_list_is_an_error() {
    let (leg_a, _) = reference_payloads();

    let update = update_with_loaded(
        vec![filler_key(1)],
        vec![filler_key(2)],
        vec![filler_key(3)],
        vec![group(0, vec![instruction(9, leg_a)])],
    );

    let error = from_grpc(&update, reference_timestamp()).expect_err("index 9 of 3 keys");
    let message = error.to_string();
    assert!(message.contains('9'), "{message}");
    assert!(
        message.contains('3'),
        "the error should say how many keys there were: {message}"
    );
}

/// Every inner instruction becomes a payload, whatever program it names. The
/// filter on the emitting program runs downstream, and an adapter that narrowed
/// here would renumber stored events — `InnerInstructionPayload` calls this
/// "only ever widen".
///
/// This is what the reduced reference rendering above cannot show, since it
/// carries cp-amm instructions only.
#[test]
fn every_inner_instruction_is_kept_whatever_its_program() {
    let (leg_a, leg_b) = reference_payloads();
    let program = Protocol::MeteoraDammV2.program_id();
    let stranger = filler_key(7);

    let update = update_with(
        vec![program.to_bytes().to_vec(), stranger.clone()],
        vec![
            group(
                5,
                vec![instruction(0, leg_a), instruction(1, vec![1, 2, 3])],
            ),
            group(8, vec![instruction(1, vec![4, 5]), instruction(0, leg_b)]),
        ],
    );

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");

    assert_eq!(
        actual.inner_instructions.len(),
        4,
        "the two instructions addressed elsewhere must survive"
    );
    let programs: Vec<_> = actual
        .inner_instructions
        .iter()
        .map(|p| p.program_id)
        .collect();
    let other = Pubkey::try_from(stranger.as_slice()).expect("32 bytes");
    assert_eq!(programs, vec![program, other, other, program]);
}

// ── the position fields ─────────────────────────────────────────────

/// The index is narrowed, never truncated. A wrapped value would not fail — it
/// would order events wrongly in a stored column, which is the defect this whole
/// migration exists to remove.
#[test]
fn an_index_too_large_for_the_domain_is_an_error() {
    let program = Protocol::MeteoraDammV2.program_id();
    let mut update = update_with(vec![program.to_bytes().to_vec()], Vec::new());
    update.transaction.as_mut().expect("built with one").index = u64::from(u32::MAX) + 1;

    let error = from_grpc(&update, reference_timestamp()).expect_err("index overflows u32");
    assert!(error.to_string().contains("u32"), "{error}");
}

#[test]
fn a_missing_transaction_envelope_is_an_error() {
    let update = SubscribeUpdateTransaction {
        slot: REFERENCE_SLOT,
        transaction: None,
    };

    let error = from_grpc(&update, reference_timestamp()).expect_err("no transaction");
    assert!(error.to_string().contains("transaction"), "{error}");
}

#[test]
fn a_signature_of_the_wrong_length_is_an_error() {
    let program = Protocol::MeteoraDammV2.program_id();
    let mut update = update_with(vec![program.to_bytes().to_vec()], Vec::new());
    update
        .transaction
        .as_mut()
        .expect("built with one")
        .signature = vec![0u8; 31];

    let error = from_grpc(&update, reference_timestamp()).expect_err("31-byte signature");
    assert!(error.to_string().contains("31"), "{error}");
}

/// ⚠️ A message that is absent must **refuse**, not resolve against zero static
/// keys. Found in review, 8 September 2026: an empty first segment shifts every
/// index one segment along, so `program_id_index = 0` lands on the first
/// *loaded* key — a valid, wrong `Pubkey` the downstream filter drops in
/// silence. The loaded segments below are non-empty precisely so that a
/// regression resolves to something instead of failing on its own.
#[test]
fn a_missing_message_is_an_error_not_an_empty_key_list() {
    let (leg_a, _) = reference_payloads();
    let program = Protocol::MeteoraDammV2.program_id();

    let mut update = update_with_loaded(
        vec![filler_key(1)],
        vec![program.to_bytes().to_vec()],
        vec![filler_key(3)],
        vec![group(0, vec![instruction(0, leg_a)])],
    );
    update
        .transaction
        .as_mut()
        .expect("built with one")
        .transaction = None;

    let error = from_grpc(&update, reference_timestamp()).expect_err("no message");
    assert!(error.to_string().contains("message"), "{error}");
}

/// ⚠️ `inner_instructions_none` means "the source did not capture them", which
/// is not "there were none". Reading it as an empty list records a transaction
/// full of events as "nothing to record" — silently, for ever. Refusing puts it
/// on the skip-and-log path, where it is counted.
#[test]
fn inner_instructions_not_captured_is_an_error_not_an_empty_list() {
    let program = Protocol::MeteoraDammV2.program_id();
    let mut update = update_with(vec![program.to_bytes().to_vec()], Vec::new());
    update
        .transaction
        .as_mut()
        .expect("built with one")
        .meta
        .as_mut()
        .expect("built with one")
        .inner_instructions_none = true;

    let error = from_grpc(&update, reference_timestamp()).expect_err("not captured");
    assert!(
        error.to_string().contains("not captured"),
        "the error must distinguish absence from emptiness: {error}"
    );
}

// ── absences that are not failures ──────────────────────────────────

/// A transaction with no inner instructions is ordinary — most are. It yields an
/// empty payload list, and extraction reports "nothing to record".
#[test]
fn no_inner_instructions_is_an_empty_list_not_an_error() {
    let program = Protocol::MeteoraDammV2.program_id();
    let update = update_with(vec![program.to_bytes().to_vec()], Vec::new());

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");
    assert!(actual.inner_instructions.is_empty());
}

/// And a message with no `meta` at all — the field is optional on the wire.
#[test]
fn a_missing_meta_is_an_empty_list_not_an_error() {
    let program = Protocol::MeteoraDammV2.program_id();
    let mut update = update_with(vec![program.to_bytes().to_vec()], Vec::new());
    update.transaction.as_mut().expect("built with one").meta = None;

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");
    assert!(actual.inner_instructions.is_empty());
}
