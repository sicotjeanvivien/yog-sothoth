//! Tests for the sort itself. What each path accepts, and what it says when it
//! refuses, is tested beside each listener — those are the statements that
//! differ.
//!
//! Case and the RFC 3986 shape of a scheme are [`SecretUrl::scheme`]'s, and
//! tested in `yog-bootstrap`.

use super::*;

const ACCEPTED: &[&str] = &["ws", "wss"];

fn url(raw: &str) -> SecretUrl {
    SecretUrl::for_tests(raw)
}

#[test]
fn a_scheme_in_the_accepted_set_passes() {
    assert_eq!(check(&url("wss://api.example.com"), ACCEPTED), Ok(()));
}

#[test]
fn a_foreign_scheme_is_refused_by_name() {
    assert_eq!(
        check(&url("https://grpc.example.com:443"), ACCEPTED),
        Err(SchemeRefusal::Foreign("https".to_string()))
    );
}

#[test]
fn a_url_without_a_scheme_is_refused_as_missing() {
    assert_eq!(
        check(&url("api.example.com:443"), ACCEPTED),
        Err(SchemeRefusal::Missing)
    );
}
