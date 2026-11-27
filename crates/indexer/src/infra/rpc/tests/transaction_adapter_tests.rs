//! The transaction-level refusals of the RPC adapter, each built by removing exactly one thing
//! from a real mainnet fixture; the happy path is the whole corpus's.
//! ⚠️ The refusals and `no_inner_instructions_…` hold the distinction together: a refusal that
//! also swallowed a genuine `[]` would pass every other test here.

use super::*;

fn fixture_json() -> serde_json::Value {
    named_fixture_json("swap_ok.json")
}

fn named_fixture_json(name: &str) -> serde_json::Value {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures/damm_v2")
        .join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("fixture is not valid JSON")
}

fn parse(value: serde_json::Value) -> EncodedConfirmedTransactionWithStatusMeta {
    serde_json::from_value(value).expect("fixture is not a valid RPC transaction")
}

#[test]
fn unmodified_fixture_is_accepted() {
    // The control: without it the tests below could be green on a fixture that never converted.
    let on_chain_tx = from_rpc(&parse(fixture_json())).expect("the untouched fixture must convert");
    assert!(
        !on_chain_tx.inner_instructions.is_empty(),
        "the fixture must carry inner instructions for the refusals below to mean anything"
    );
}

#[test]
fn a_transaction_without_block_time_is_refused() {
    let mut json = fixture_json();
    json.as_object_mut().unwrap().remove("blockTime");

    let err = from_rpc(&parse(json)).expect_err("a missing blockTime must be refused");

    assert!(
        matches!(&err, TransactionAdapterError::MissingField { field, .. } if field == "blockTime"),
        "expected a MissingField on blockTime, got {err:?}"
    );
}

#[test]
fn a_transaction_in_another_encoding_is_refused() {
    let mut json = fixture_json();
    // Not `Json`: a bare string deserializes as the legacy binary encoding, with no readable signature.
    json.as_object_mut().unwrap().insert(
        "transaction".to_string(),
        serde_json::Value::String("AQAB".to_string()),
    );

    let err = from_rpc(&parse(json)).expect_err("a non-JSON encoding must be refused");

    assert!(
        matches!(&err, TransactionAdapterError::Parse { reason, .. } if reason.contains("encoding")),
        "expected a ParseError about the encoding, got {err:?}"
    );
}

// ── absence is not emptiness ────────────────────────────────────────

/// ⚠️ An absent `meta` is "not carried", never "none": refused and counted. Serde's flatten turns
/// the missing key into `None` silently, so nothing upstream raises.
#[test]
fn meta_not_captured_is_an_error_not_an_empty_list() {
    let mut json = fixture_json();
    json.as_object_mut().unwrap().remove("meta");

    let err = from_rpc(&parse(json)).expect_err("an absent meta must be refused");

    // The exact field, not `contains("not captured")`: which absence occurred is what the operator acts on.
    assert!(
        matches!(&err, TransactionAdapterError::MissingField { field, .. }
            if field == Gap::Meta.field()),
        "the error must distinguish absence from emptiness, and name which: {err:?}"
    );
}

/// The same absence one level down, answered the same way; `null` also covers the removed key,
/// both deserializing to `OptionSerializer::None`.
#[test]
fn inner_instructions_not_captured_is_an_error_not_an_empty_list() {
    let mut json = fixture_json();
    json["meta"]["innerInstructions"] = serde_json::Value::Null;

    let err = from_rpc(&parse(json)).expect_err("absent innerInstructions must be refused");

    assert!(
        matches!(&err, TransactionAdapterError::MissingField { field, .. }
            if field == Gap::InnerInstructions.field()),
        "the error must distinguish absence from emptiness, and name which: {err:?}"
    );
}

/// A transaction that genuinely carries no inner instructions is ordinary: an empty payload list.
/// ⚠️ The half that gives the two refusals above their meaning.
#[test]
fn no_inner_instructions_is_an_empty_list_not_an_error() {
    let mut json = fixture_json();
    json["meta"]["innerInstructions"] = serde_json::Value::Array(Vec::new());

    let on_chain_tx = from_rpc(&parse(json)).expect("an empty group list is not a failure");

    assert!(
        on_chain_tx.inner_instructions.is_empty(),
        "an empty group list must yield an empty payload list"
    );
}

// ── unreadable inner instructions ───────────────────────────────────

/// The first `PartiallyDecoded` inner instruction of the fixture, the only shape the adapter reads.
fn first_partially_decoded(json: &mut serde_json::Value) -> &mut serde_json::Value {
    json["meta"]["innerInstructions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .flat_map(|g| g["instructions"].as_array_mut().unwrap().iter_mut())
        .find(|ix| ix.get("data").is_some())
        .expect("the fixture must carry a partially decoded inner instruction")
}

/// Mutation: an unreadable program id dropped from the payloads, which renumbers every later event.
#[test]
fn an_inner_instruction_with_an_unreadable_program_id_refuses_the_transaction() {
    let mut json = fixture_json();
    first_partially_decoded(&mut json)["programId"] = "not-a-pubkey".into();

    let err = from_rpc(&parse(json)).expect_err("an unreadable program id must be refused");

    assert!(
        matches!(&err, TransactionAdapterError::Parse { reason, .. }
            if reason.starts_with("inner instruction program id not-a-pubkey")),
        "expected a ParseError naming the program id, got {err:?}"
    );
}

/// Mutation: an indexed program's undecodable data dropped, which renumbers every later event.
#[test]
fn an_indexed_programs_inner_instruction_whose_data_is_not_base58_refuses_the_transaction() {
    let mut json = fixture_json();
    let ix = first_partially_decoded(&mut json);
    ix["programId"] = Protocol::MeteoraDammV2.program_id().to_string().into();
    // `0`, `O`, `I` and `l` are outside the base58 alphabet.
    ix["data"] = "0OIl".into();

    let err = from_rpc(&parse(json)).expect_err("non-base58 data must be refused");

    assert!(
        matches!(&err, TransactionAdapterError::Parse { reason, .. }
        if reason.starts_with(&format!(
            "inner instruction data of {} is not base58",
            Protocol::MeteoraDammV2.program_id()
        ))),
        "expected a ParseError naming the program, got {err:?}"
    );
}

/// A foreign program's payload never numbers an event of ours: it is set aside, not refused.
/// Mutation: refusing it, which loses every Meteora event of the transaction.
#[test]
fn a_foreign_programs_inner_instruction_whose_data_is_not_base58_is_set_aside() {
    let whole = from_rpc(&parse(fixture_json())).expect("fixture must convert");
    let mut json = fixture_json();
    let ix = first_partially_decoded(&mut json);
    ix["programId"] = "11111111111111111111111111111111".into();
    ix["data"] = "0OIl".into();

    let on_chain_tx = from_rpc(&parse(json)).expect("a foreign program's data cannot refuse");

    assert_eq!(
        on_chain_tx.inner_instructions.len(),
        whole.inner_instructions.len() - 1
    );
}

/// An instruction the node fully parsed (a token transfer) is set aside, not refused.
/// Mutation: refusing anything that is not `PartiallyDecoded`, which refuses ordinary swaps.
#[test]
fn an_inner_instruction_parsed_by_the_node_is_set_aside() {
    let json = fixture_json();
    let instructions: Vec<_> = json["meta"]["innerInstructions"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["instructions"].as_array().unwrap())
        .collect();
    let partially_decoded = instructions
        .iter()
        .filter(|ix| ix.get("data").is_some())
        .count();
    assert!(
        partially_decoded < instructions.len(),
        "the fixture must carry a parsed instruction for this test to mean anything"
    );

    let on_chain_tx = from_rpc(&parse(json)).expect("parsed instructions are ordinary traffic");

    assert_eq!(on_chain_tx.inner_instructions.len(), partially_decoded);
}

/// The payload order does not depend on the order the groups were serialized in.
/// Mutation this is written against: removing `sort_by_key` in `extract_inner_instructions`; every
/// fixture arrives sorted, so the groups (0 and 2 of `initialize_reward.json`) are reversed here.
#[test]
fn group_order_from_the_source_does_not_change_the_payload_order() {
    let json = named_fixture_json("initialize_reward.json");

    let groups = json["meta"]["innerInstructions"]
        .as_array()
        .expect("the fixture must carry inner instructions");
    assert!(
        groups.len() >= 2,
        "this test needs a multi-group transaction, got {} group(s)",
        groups.len()
    );

    let mut shuffled = json.clone();
    shuffled["meta"]["innerInstructions"]
        .as_array_mut()
        .unwrap()
        .reverse();

    let expected = from_rpc(&parse(json)).expect("fixture must convert");
    let actual = from_rpc(&parse(shuffled)).expect("reversed fixture must convert");

    assert_eq!(
        actual.inner_instructions, expected.inner_instructions,
        "the payload order followed the order the groups were serialized in"
    );
}

/// ⚠️ The one test tying `conformance`'s hand-written reference to a real mainnet response.
/// Mutation this is written against: swapping the two payloads in `conformance::reference_transaction`.
#[test]
fn the_reference_transaction_is_what_this_adapter_produces() {
    let json = named_fixture_json("swap_double.json");

    let on_chain_tx = from_rpc(&parse(json)).expect("the reference fixture must convert");

    yog_core::application::extraction::conformance::assert_matches_reference(
        &on_chain_tx,
        // The provider in use leaves `transaction_index` out.
        None,
    );
}
