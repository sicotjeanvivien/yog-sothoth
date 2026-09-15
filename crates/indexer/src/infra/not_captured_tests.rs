//! The one guard a single definition cannot give itself.
//!
//! Every other test of these two labels — four of them, two per adapter —
//! compares what an adapter produced against the constants below. That catches
//! an adapter naming the wrong absence, and it makes the two adapters agree by
//! construction. What it cannot catch is the constants themselves being
//! swapped: adapters and tests would move together and stay green, with both
//! absences reported under each other's name everywhere at once.
//!
//! So this suite pins the text at the one site that defines it. It is a
//! change-detector, knowingly: these strings are what an operator greps for
//! when a provider stops capturing, so rewording one is a decision to take
//! here, not a rename to carry out elsewhere.

use super::*;

#[test]
fn the_two_labels_are_the_words_operators_read() {
    assert_eq!(META, "meta (not captured by the source)");
    assert_eq!(
        INNER_INSTRUCTIONS,
        "meta.inner_instructions (not captured by the source)"
    );
}

/// The refusal carries the transaction as well as the field.
///
/// The signature is the only identifier either log line has for the thing it is
/// stepping over — `fetch_worker` and `session` both print it — so a refusal
/// that dropped it would leave an operator with a reason and nothing to apply
/// it to.
#[test]
fn the_refusal_names_the_field_and_the_transaction() {
    let signature = Signature::from([7u8; 64]);

    let error = refuse(META, &signature);

    assert!(
        matches!(&error, CoreError::MissingField { field, signature: s }
            if field == META && s == &signature.to_string()),
        "expected a MissingField naming both: {error:?}"
    );
}
