//! Unit tests for the RFC 9457 ProblemDetails wire shape.
//!
//! The DTO carries the public error format. These tests pin the
//! exact field names and the discriminator value (`about:blank`)
//! so they fail at `cargo test` time if a refactor accidentally
//! changes the contract.

use super::ProblemDetails;

#[test]
fn serialises_all_four_required_fields() {
    let p = ProblemDetails::generic("Bad Request", 400, "invalid pool address".to_string());
    let json = serde_json::to_value(&p).unwrap();

    // The four RFC 9457 fields, in their wire form.
    assert!(json.get("type").is_some(), "missing field `type`");
    assert!(json.get("title").is_some(), "missing field `title`");
    assert!(json.get("status").is_some(), "missing field `status`");
    assert!(json.get("detail").is_some(), "missing field `detail`");
}

#[test]
fn type_field_uses_rfc_name_not_rust_name() {
    // The Rust struct field is `type_uri` to avoid the keyword
    // collision; the wire field must still be `type`.
    let p = ProblemDetails::generic("Bad Request", 400, "x".to_string());
    let json = serde_json::to_value(&p).unwrap();

    assert!(json.get("type").is_some());
    assert!(json.get("type_uri").is_none(), "rust field name leaked");
}

#[test]
fn generic_constructor_sets_type_to_about_blank() {
    // Public contract: `generic(...)` always emits `"about:blank"`.
    // When specific URIs are introduced later, they will live in a
    // new constructor (`with_type(...)`); this one must remain the
    // safe default for unclassified errors.
    let p = ProblemDetails::generic("Bad Request", 400, "x".to_string());
    let json = serde_json::to_value(&p).unwrap();

    assert_eq!(json["type"], "about:blank");
}

#[test]
fn status_is_serialised_as_number() {
    // RFC 9457 §3.1.3: `status` MUST be a number, not a string.
    let p = ProblemDetails::generic("Bad Request", 400, "x".to_string());
    let json = serde_json::to_value(&p).unwrap();

    assert!(json["status"].is_number(), "`status` must be a number");
    assert_eq!(json["status"], 400);
}

#[test]
fn detail_carries_per_occurrence_message() {
    let p = ProblemDetails::generic("Bad Request", 400, "invalid pool address: foo".to_string());
    let json = serde_json::to_value(&p).unwrap();

    assert_eq!(json["detail"], "invalid pool address: foo");
}

#[test]
fn title_is_stable_per_problem_type() {
    // Two errors of the same kind must produce the same `title`,
    // even when their `detail` differs. This is what clients can
    // branch on for type-based discrimination at the `about:blank`
    // stage.
    let p1 = ProblemDetails::generic("Bad Request", 400, "msg one".to_string());
    let p2 = ProblemDetails::generic("Bad Request", 400, "msg two".to_string());

    let j1 = serde_json::to_value(&p1).unwrap();
    let j2 = serde_json::to_value(&p2).unwrap();

    assert_eq!(j1["title"], j2["title"]);
    assert_ne!(j1["detail"], j2["detail"]);
}
