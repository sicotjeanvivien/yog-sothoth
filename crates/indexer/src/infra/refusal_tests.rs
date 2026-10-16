//! The one guard a single definition cannot give itself: the adapters' tests compare against the
//! constants, so swapped constants would move adapters and tests together.
//! ⚠️ A change-detector on purpose: rewording what an operator greps for is a decision taken here.

use super::*;

#[test]
fn the_two_labels_are_the_words_operators_read() {
    assert_eq!(Gap::Meta.field(), "meta (not captured by the source)");
    assert_eq!(
        Gap::InnerInstructions.field(),
        "meta.inner_instructions (not captured by the source)"
    );
}

/// The refusal carries the transaction as well as the field: the signature is all either log line
/// has to apply the reason to.
#[test]
fn the_refusal_names_the_field_and_the_transaction() {
    let signature = Signature::from([7u8; 64]);

    let error = refuse(Gap::Meta, &signature);

    assert!(
        matches!(&error, CoreError::MissingField { field, signature: s }
            if field == Gap::Meta.field() && s == &signature.to_string()),
        "expected a MissingField naming both: {error:?}"
    );
}
