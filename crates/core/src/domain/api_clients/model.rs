//! A client of the API and the key it presents: storage only ever sees the key's hash.

use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

/// What a key is attached to, and what its calls are counted under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiClient {
    pub id: i64,
    pub name: String,
}

/// A key the API accepts: its hash, and the client it opens the API to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveKey {
    pub hash: [u8; 32],
    pub client: ApiClient,
}

pub const API_KEY_PREFIX_BYTES: usize = 4;
pub const API_KEY_SECRET_BYTES: usize = 32;

const MARKER: &str = "yog_";

/// `yog_<prefix>_<secret>`, both in lowercase hex. The prefix names the key to an operator;
/// ⚠️ `Debug` shows it alone, never the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey {
    text: String,
}

/// Why a presented string is not an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ApiKeyFormatError {
    #[error("an API key starts with `yog_`")]
    Marker,
    #[error("an API key is `yog_<prefix>_<secret>`")]
    Parts,
    #[error("the prefix of an API key is {} hex characters", API_KEY_PREFIX_BYTES * 2)]
    PrefixLength,
    #[error("the secret of an API key is {} hex characters", API_KEY_SECRET_BYTES * 2)]
    SecretLength,
    #[error("an API key is lowercase hex after its marker")]
    NotLowerHex,
}

impl ApiKey {
    /// A new key from random bytes; the caller draws them, this crate does no I/O.
    pub fn from_random(
        prefix: [u8; API_KEY_PREFIX_BYTES],
        secret: [u8; API_KEY_SECRET_BYTES],
    ) -> Self {
        Self {
            text: format!("{MARKER}{}_{}", lower_hex(&prefix), lower_hex(&secret)),
        }
    }

    pub fn parse(text: &str) -> Result<Self, ApiKeyFormatError> {
        let rest = text.strip_prefix(MARKER).ok_or(ApiKeyFormatError::Marker)?;
        let (prefix, secret) = rest.split_once('_').ok_or(ApiKeyFormatError::Parts)?;
        if prefix.len() != API_KEY_PREFIX_BYTES * 2 {
            return Err(ApiKeyFormatError::PrefixLength);
        }
        if secret.len() != API_KEY_SECRET_BYTES * 2 {
            return Err(ApiKeyFormatError::SecretLength);
        }
        if !prefix.bytes().chain(secret.bytes()).all(is_lower_hex) {
            return Err(ApiKeyFormatError::NotLowerHex);
        }
        Ok(Self {
            text: text.to_string(),
        })
    }

    pub fn prefix(&self) -> &str {
        &self.text[MARKER.len()..MARKER.len() + API_KEY_PREFIX_BYTES * 2]
    }

    /// The whole key, secret included: to hand to its client once, and never to log.
    pub fn expose(&self) -> &str {
        &self.text
    }

    /// The SHA-256 of the whole key, what storage keeps.
    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.text.as_bytes()).into()
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey({MARKER}{}_…)", self.prefix())
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
