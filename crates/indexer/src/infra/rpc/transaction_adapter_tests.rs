//! The transaction-level refusals of the RPC adapter — and the one absence
//! that is not a refusal.
//!
//! The happy path is covered by the whole fixture corpus (the two sibling
//! suites: `fixture_pipeline_tests` and `extraction_oracle_tests`). What no
//! fixture exercises is what the adapter *rejects*, and each refusal matters
//! for its own reason: `timestamp` is a partitioning column and part of every
//! event's unique key, so a transaction without one must not reach extraction
//! at all; an uncaptured `meta` or `innerInstructions` would otherwise pass for
//! a transaction with nothing in it.
//!
//! ⚠️ **The refusals are only half of that last pair.** `no_inner_instructions_…`
//! pins the case that must keep flowing, and without it a refusal that also
//! swallowed a genuine `[]` would pass every other test here. Whether the
//! distinction holds is what these three assert together — never one of them
//! alone.
//!
//! Built by taking a real mainnet fixture and removing exactly one thing, so
//! the test cannot pass because the transaction was malformed some other way.
//! The fixtures stay in `yog-core` and are read from here by path — their value
//! is being the verbatim RPC response, and a second copy would drift.

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
    // The control: without it, the two tests below could be green because the
    // fixture never converted in the first place.
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
        matches!(&err, CoreError::MissingField { field, .. } if field == "blockTime"),
        "expected a MissingField on blockTime, got {err:?}"
    );
}

#[test]
fn a_transaction_in_another_encoding_is_refused() {
    let mut json = fixture_json();
    // Anything but the `Json` variant: a bare string deserializes as the
    // legacy binary encoding, which carries no readable signature.
    json.as_object_mut().unwrap().insert(
        "transaction".to_string(),
        serde_json::Value::String("AQAB".to_string()),
    );

    let err = from_rpc(&parse(json)).expect_err("a non-JSON encoding must be refused");

    assert!(
        matches!(&err, CoreError::ParseError { reason, .. } if reason.contains("encoding")),
        "expected a ParseError about the encoding, got {err:?}"
    );
}

// ── absence is not emptiness ────────────────────────────────────────

/// ⚠️ An absent `meta` says "the response does not carry the inner
/// instructions", never "there were none". Reading it as an empty list records
/// a transaction full of events as "nothing to record" — silently, for ever, on
/// the one ingestion path that runs today. Refusing puts it on the skip-and-log
/// path, where it is counted.
///
/// It is quieter than the other refusals of this module: serde's flatten on the
/// enclosing type makes the missing key deserialize to `None` rather than fail,
/// so nothing upstream raises either.
#[test]
fn meta_not_captured_is_an_error_not_an_empty_list() {
    let mut json = fixture_json();
    json.as_object_mut().unwrap().remove("meta");

    let err = from_rpc(&parse(json)).expect_err("an absent meta must be refused");

    // The exact field, not `contains("not captured")`: the two refusals of this
    // pair differ only by *which* absence occurred, and that is the whole of
    // what the operator acts on — a provider that dropped `meta` wholesale and
    // one that stopped recording inner instructions are two different fixes.
    // Asserting the looser predicate let the two labels be swapped with all 153
    // tests green; checked by mutation, 15 September 2026.
    assert!(
        matches!(&err, CoreError::MissingField { field, .. }
            if field == META),
        "the error must distinguish absence from emptiness, and name which: {err:?}"
    );
}

/// The same absence one level down, and it must be answered the same way.
///
/// `null` rather than a removed key on purpose: both deserialize to
/// `OptionSerializer::None` — the field carries
/// `default = "OptionSerializer::none"` — so this case covers the removed key
/// too, and `OptionSerializer::Skip` is not reachable from the wire at all.
#[test]
fn inner_instructions_not_captured_is_an_error_not_an_empty_list() {
    let mut json = fixture_json();
    json["meta"]["innerInstructions"] = serde_json::Value::Null;

    let err = from_rpc(&parse(json)).expect_err("absent innerInstructions must be refused");

    assert!(
        matches!(&err, CoreError::MissingField { field, .. }
            if field == INNER_INSTRUCTIONS),
        "the error must distinguish absence from emptiness, and name which: {err:?}"
    );
}

/// A transaction that genuinely carries no inner instructions is ordinary. It
/// yields an empty payload list, and extraction reports "nothing to record".
///
/// ⚠️ **This is the half that makes the two above mean something.** A refusal
/// that also swallowed `[]` would not have distinguished anything — it would
/// have moved the confusion, not removed it.
///
/// And `[]` is ordinary mainnet traffic on this very path: an invocation that
/// simply makes no CPI. The corpus witnesses it directly — six of the 74 `dlmm`
/// fixtures, `close_bin_array.json` among them with six invocations and no
/// inner instruction at all.
///
/// (An ALT-only reference produces an empty list too, but never *here*: with no
/// `Program … invoke` line it is rejected by `InvocationFilter` before the
/// fetch. That cause belongs to the gRPC path, which has no such filter.)
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

/// The payload order does not depend on the order the source serialized the
/// inner-instruction groups in.
///
/// This is the invariant `groups.sort_by_key(|g| g.index)` exists for, and the
/// fixture corpus does not witness it: every mainnet fixture already arrives
/// with its groups in ascending order, so deleting the sort leaves the whole
/// suite green. Here the groups are handed over reversed on purpose.
///
/// Mutation-checked: remove the `sort_by_key` in `extract_inner_instructions`
/// and this test fails with the two payload lists differing — without that
/// check it would be asserting nothing.
///
/// `initialize_reward.json` is used because it is one of the two fixtures whose
/// payloads actually span **several** groups (0 and 2). On a single-group
/// transaction reversing the list is a no-op and the test would be vacuous —
/// hence the assertion on the group count below.
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

/// **The pin.** The reference expectation in `conformance` is hand-written; this
/// is the one test that ties it to a real mainnet response, and every other
/// consumer of that expectation — a future protobuf adapter, the persistence
/// integration test — trusts it because of this assertion.
///
/// Delete it and `conformance` becomes what its own doc-comment warns against:
/// a transcription agreeing with the code that reads it.
///
/// Mutation-checked: swap the two payloads in `conformance::reference_transaction`
/// and this fails with `payload 0: bytes differ`.
#[test]
fn the_reference_transaction_is_what_this_adapter_produces() {
    let json = named_fixture_json("swap_double.json");

    let on_chain_tx = from_rpc(&parse(json)).expect("the reference fixture must convert");

    yog_core::application::extraction::conformance::assert_matches_reference(
        &on_chain_tx,
        // What this source provides: nothing. `getTransaction` leaves
        // `transaction_index` out, which is what the gRPC migration is for.
        None,
    );
}
