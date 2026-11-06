//! DB-free tests of `TryFrom<ActiveKeyRow> for ActiveKey`.

use yog_core::{
    RepositoryError,
    domain::{ActiveKey, ApiClient},
};

use super::ActiveKeyRow;

fn row(key_hash: Vec<u8>) -> ActiveKeyRow {
    ActiveKeyRow {
        key_hash,
        client_id: 7,
        client_name: "dashboard".to_string(),
    }
}

/// Mutation: the client's id or name taken from anywhere but its row.
#[test]
fn a_row_becomes_its_key_and_its_client() {
    let key = ActiveKey::try_from(row(vec![9; 32])).expect("a 32-byte hash");

    assert_eq!(
        key,
        ActiveKey {
            hash: [9; 32],
            client: ApiClient {
                id: 7,
                name: "dashboard".to_string(),
            },
        }
    );
}

/// Mutation: a short hash padded or truncated into a key instead of refused.
#[test]
fn a_hash_of_another_size_is_an_integrity_error() {
    let refused = ActiveKey::try_from(row(vec![9; 31])).expect_err("a 31-byte hash");

    assert!(
        matches!(&refused, RepositoryError::Integrity(m) if m == "an api key hash of 31 bytes"),
        "{refused:?}"
    );
}
