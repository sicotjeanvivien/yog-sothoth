use super::*;

#[test]
fn tail_keeps_the_end_of_a_long_message() {
    let long = format!("{}END", "x".repeat(2000));
    let kept = tail(&long);
    assert!(kept.ends_with("END"));
    assert_eq!(kept.chars().count(), MESSAGE_TAIL);
}
