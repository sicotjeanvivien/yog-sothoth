//! Tests for the Yellowstone protobuf adapter; the inputs are hand-built, see its module doc.
//! ⚠️ The reference rendering keeps only the two cp-amm inner instructions of the fourteen: the
//! JSON-RPC adapter keeps two, and the arbiter compares whole vectors. That every program is kept
//! is [`every_inner_instruction_is_kept_whatever_its_program`]'s.

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

/// The position in the slot a provider would report; any value, stated rather than read back.
const REFERENCE_INDEX: u64 = 42;

/// The timestamp the arbiter expects; that the adapter uses what it is handed is
/// [`the_timestamp_is_the_one_supplied`]'s.
fn reference_timestamp() -> DateTime<Utc> {
    reference_transaction().position.timestamp
}

fn signature_bytes() -> Vec<u8> {
    bs58::decode(REFERENCE_SIGNATURE)
        .into_vec()
        .expect("the reference signature is a constant and must decode")
}

/// A pubkey that is no real program, for the account slots the tests do not care about.
fn filler_key(seed: u8) -> Vec<u8> {
    vec![seed; 32]
}

/// Build an update with exactly `static_keys`, the given inner-instruction groups, no loaded addresses.
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
        bank_id: 0,
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

/// The payloads the arbiter expects, taken from the arbiter rather than transcribed a third time.
fn reference_payloads() -> (Vec<u8>, Vec<u8>) {
    let tx = reference_transaction();
    (
        tx.inner_instructions[0].data.clone(),
        tx.inner_instructions[1].data.clone(),
    )
}

// ── the shared contract ─────────────────────────────────────────────

/// The arbiter both adapters answer to; the payloads sit in groups 5 and 8, as on chain, so a
/// flattening that ignored group order fails here.
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

/// Groups handed back in descending order still produce ascending payloads: providers need not
/// sort, and `event_index` is a stored key.
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

/// The timestamp is the one handed in, never read off the message, which carries none.
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

/// ⚠️ No fixture carries `loadedAddresses`, so this is the only resolution test. The program sits
/// in `loaded_readonly` behind a non-empty `loaded_writable`, the one layout that tells both wrong orders.
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

/// An index past the end is an error, not an absent payload that would renumber later events.
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
    // ⚠️ Whole phrases, not the digits `9` and `3`: the signature in every error's Display
    // already contains both.
    assert!(
        message.contains("program_id_index 9"),
        "the error must name the index it could not resolve: {message}"
    );
    assert!(
        message.contains("3 account keys"),
        "and how many keys there were, which is what tells an operator whether \
         the message or the segments are wrong: {message}"
    );
}

/// Every inner instruction becomes a payload, whatever program it names: narrowing here would
/// renumber stored events.
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

/// The index is narrowed, never truncated: a wrapped value would order events wrongly.
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
        bank_id: 0,
    };

    let error = from_grpc(&update, reference_timestamp()).expect_err("no transaction");
    // Backticked: `MissingField`'s Display carries the bare word "transaction" in its template.
    assert!(
        error.to_string().contains("`transaction`"),
        "the error must name the field that was missing: {error}"
    );
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

/// ⚠️ An absent message must refuse, not resolve against zero static keys; the loaded segments are
/// non-empty so that a regression resolves to a wrong key instead of failing by itself.
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
    let rendered = error.to_string();
    assert!(
        rendered.contains("`transaction.message`"),
        "the error must name the field: {rendered}"
    );
    // ⚠️ And name the transaction: it is all an operator has on the skip-and-log path.
    assert!(
        rendered.contains(REFERENCE_SIGNATURE),
        "the error must say which transaction it is about: {rendered}"
    );
}

/// No `meta` refuses like `inner_instructions_none`: two ways of saying "not captured".
#[test]
fn a_missing_meta_is_an_error_not_an_empty_list() {
    let program = Protocol::MeteoraDammV2.program_id();
    let mut update = update_with(vec![program.to_bytes().to_vec()], Vec::new());
    update.transaction.as_mut().expect("built with one").meta = None;

    let error = from_grpc(&update, reference_timestamp()).expect_err("no meta");

    // ⚠️ The exact field, not `contains("not captured")`, true of both refusals: which one is
    // what an operator acts on, and only the log line tells them apart.
    assert!(
        matches!(&error, CoreError::MissingField { field, .. }
            if field == Gap::Meta.field()),
        "the error must distinguish absence from emptiness, and name which: {error:?}"
    );
}

/// ⚠️ `inner_instructions_none` means "not captured", not "none": refused and counted.
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
        matches!(&error, CoreError::MissingField { field, .. }
            if field == Gap::InnerInstructions.field()),
        "the error must distinguish absence from emptiness, and name which: {error:?}"
    );
}

// ── absences that are not failures ──────────────────────────────────

/// A transaction with no inner instructions is ordinary: an empty payload list.
#[test]
fn no_inner_instructions_is_an_empty_list_not_an_error() {
    let program = Protocol::MeteoraDammV2.program_id();
    let update = update_with(vec![program.to_bytes().to_vec()], Vec::new());

    let actual = from_grpc(&update, reference_timestamp()).expect("a well-formed update");
    assert!(actual.inner_instructions.is_empty());
}
