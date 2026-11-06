use super::*;

/// Mutation: an id recorded but never looked up, so a token opens twice.
#[test]
fn an_id_is_consumed_once() {
    let mut consumed = Consumed::default();

    assert!(consumed.consume("a", 130, 100));
    assert!(!consumed.consume("a", 130, 101));
}

/// Mutation: the purge removed, so the registry grows with every opening.
#[test]
fn an_expired_id_is_forgotten() {
    let mut consumed = Consumed::default();
    consumed.consume("a", 130, 100);

    assert!(consumed.consume("a", 170, 140), "`a` expired at 130");
}
