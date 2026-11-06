use thiserror::Error;

use crate::domain::{API_KEY_PREFIX_BYTES, API_KEY_SECRET_BYTES};

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
