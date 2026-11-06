use yog_core::domain::ActiveKey;

use super::*;

fn key(byte: u8) -> ApiKey {
    ApiKey::from_random([byte; 4], [byte; 32])
}

fn ring() -> KeyRing {
    crate::testing::key_ring(vec![ActiveKey {
        hash: key(1).hash(),
        client: ApiClient {
            id: 7,
            name: "dashboard".to_string(),
        },
    }])
}

fn client_of(credential: &str) -> Result<String, Unauthorized> {
    identify(credential, &ring()).map(|client| client.name)
}

/// Mutation: the client looked up under anything but the bearer's key.
#[test]
fn a_bearer_of_a_known_key_names_its_client() {
    assert_eq!(
        client_of(&format!("Bearer {}", key(1).expose())).as_deref(),
        Ok("dashboard")
    );
}

/// Mutation: the scheme compared case-sensitively (RFC 9110 §11.1 makes it insensitive).
#[test]
fn the_bearer_scheme_is_read_in_any_case() {
    assert_eq!(
        client_of(&format!("bEaReR {}", key(1).expose())).as_deref(),
        Ok("dashboard")
    );
}

// One refusal per rule, each named by its reason.

#[test]
fn another_scheme_is_refused_for_its_scheme() {
    assert_eq!(
        client_of(&format!("Basic {}", key(1).expose())),
        Err(Unauthorized::Scheme)
    );
}

#[test]
fn a_bearer_that_is_no_api_key_is_malformed() {
    assert_eq!(client_of("Bearer not-a-key"), Err(Unauthorized::Malformed));
}

/// Mutation: the scheme split on a single space only, so these read as another scheme.
#[test]
fn a_bearer_without_a_token_or_after_a_tab_is_malformed_not_another_scheme() {
    assert_eq!(client_of("Bearer"), Err(Unauthorized::Malformed));
    assert_eq!(client_of("Bearer\tnot-a-key"), Err(Unauthorized::Malformed));
}

#[test]
fn a_key_the_ring_does_not_hold_is_unknown() {
    assert_eq!(
        client_of(&format!("Bearer {}", key(2).expose())),
        Err(Unauthorized::Unknown)
    );
}
