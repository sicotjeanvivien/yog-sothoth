use super::*;

/// Mutation: a rule of `is_client_name` dropped.
#[test]
fn a_client_name_is_lowercase_letters_digits_and_dashes() {
    assert!(is_client_name("dashboard"));
    assert!(is_client_name("acme-2"));
    assert!(is_client_name(&"a".repeat(63)));
}

#[test]
fn an_empty_client_name_is_refused() {
    assert!(!is_client_name(""));
}

#[test]
fn a_client_name_past_63_characters_is_refused() {
    assert!(!is_client_name(&"a".repeat(64)));
}

#[test]
fn a_client_name_starting_with_a_dash_is_refused() {
    assert!(!is_client_name("-acme"));
}

#[test]
fn a_client_name_with_an_uppercase_letter_or_a_space_is_refused() {
    assert!(!is_client_name("Acme"));
    assert!(!is_client_name("acme corp"));
}
