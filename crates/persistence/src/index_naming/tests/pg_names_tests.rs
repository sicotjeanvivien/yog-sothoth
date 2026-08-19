//! The port, checked against names Postgres actually produced.

use super::*;

#[test]
fn make_object_name_should_leave_a_short_name_untouched() {
    assert_eq!(
        make_object_name("pools", Some("protocol"), "idx"),
        "pools_protocol_idx"
    );
}

#[test]
fn make_object_name_should_reproduce_the_known_truncated_name() {
    let addition = choose_index_name_addition(&key_columns());

    let name = make_object_name(DURATION_TABLE, Some(&addition), "idx");

    assert_eq!(name, DURATION_INDEX);
    assert_eq!(name.len(), MAX_IDENTIFIER_LEN);
}

/// The suffix lands on the *label*, so `idx1` costs one more character than
/// `idx` and the column part loses one too. This is the detail that makes
/// "the first 63 characters" the wrong mental model.
#[test]
fn make_object_name_should_shorten_further_when_the_label_grows() {
    let addition = choose_index_name_addition(&key_columns());

    let plain = make_object_name(FUNDER_TABLE, Some(&addition), "idx");
    let suffixed = make_object_name(FUNDER_TABLE, Some(&addition), "idx1");

    assert_eq!(plain, DURATION_INDEX, "truncates onto the same name");
    assert_eq!(suffixed, FUNDER_INDEX);
    assert_ne!(
        plain.trim_end_matches("_idx"),
        suffixed.trim_end_matches("_idx1"),
        "the column part must lose a character, not just the label"
    );
}

/// `makeObjectName` trims the longer part first, alternating — it does not cut
/// the tail of the concatenation.
#[test]
fn make_object_name_should_trim_the_longer_part_first() {
    let long_table = "a".repeat(50);
    let long_columns = "b".repeat(50);

    let name = make_object_name(&long_table, Some(&long_columns), "idx");

    let (table_part, column_part) = name
        .trim_end_matches("_idx")
        .split_once('_')
        .expect("the two parts stay separated");
    assert_eq!(
        table_part.len(),
        column_part.len(),
        "equal-length parts must be trimmed evenly, got {name}"
    );
    assert_eq!(name.len(), MAX_IDENTIFIER_LEN);
}

#[test]
fn choose_index_name_addition_should_join_column_names_with_underscores() {
    assert_eq!(
        choose_index_name_addition(&key_columns()),
        "signature_event_index_timestamp"
    );
}

#[test]
fn choose_index_name_addition_should_stop_once_the_buffer_is_full() {
    let columns = vec!["c".repeat(40), "d".repeat(40), "e".repeat(40)];

    let addition = choose_index_name_addition(&columns);

    assert_eq!(
        addition.len(),
        81,
        "two columns and a separator, then it stops"
    );
    assert!(!addition.contains('e'), "the third column is never reached");
}

#[test]
fn choose_relation_name_should_count_the_passes_it_needed() {
    let mut taken = HashSet::new();
    taken.insert("pools_protocol_idx".to_string());

    let (name, passes) = choose_relation_name("pools", Some("protocol"), "idx", &taken);

    assert_eq!(name, "pools_protocol_idx1");
    assert_eq!(passes, 1);
}
