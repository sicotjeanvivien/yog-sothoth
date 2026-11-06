//! A client of the API: what a key is attached to, and what its calls are counted under.

/// A client, by its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiClient {
    pub id: i64,
    pub name: String,
}

/// Whether `name` may name a client: 1 to 63 of `[a-z0-9-]`, a letter or a digit first.
/// ⚠️ It becomes a metrics label, which `observe.sh` prints only in that alphabet.
pub fn is_client_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=63).contains(&bytes.len())
        && bytes[0] != b'-'
        && bytes
            .iter()
            .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
