//! The two transaction-level refusals of the RPC adapter.
//!
//! The happy path is covered by the whole fixture corpus (`tests/live_detector.rs`
//! and the behaviour oracle). What no fixture exercises is what the adapter
//! *rejects* — and both refusals matter: `timestamp` is a partitioning column
//! and part of every event's unique key, so a transaction without one must not
//! reach extraction at all.
//!
//! Built by taking a real mainnet fixture and removing exactly one thing, so
//! the test cannot pass because the transaction was malformed some other way.

use super::*;

fn fixture_json() -> serde_json::Value {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/damm_v2/swap_ok.json");
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
    let view = from_rpc(&parse(fixture_json())).expect("the untouched fixture must convert");
    assert!(
        !view.inner_instructions.is_empty(),
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
