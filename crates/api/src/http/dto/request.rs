//! Request DTOs: `XxxRequest::parse(...)` runs every validation of one endpoint once, so a
//! value cannot exist invalid. The cursor codec lives in [`http::cursor`].
//!
//! [`http::cursor`]: crate::http::cursor

use std::str::FromStr;

use solana_pubkey::Pubkey;

use crate::http::error::ApiError;

mod get_token;
mod list_signals;

pub(crate) use get_token::GetTokenRequest;
#[cfg(test)]
pub(crate) use list_signals::MAX_LIMIT;
pub(crate) use list_signals::{ListSignalsRequest, SignalsQuery};

/// `raw`, or a 400: a parameter present without a value is invalid, never absent.
fn non_empty<'a>(name: &str, raw: &'a str) -> Result<&'a str, ApiError> {
    if raw.is_empty() {
        return Err(ApiError::BadRequest(format!("`{name}` must not be empty")));
    }
    Ok(raw)
}

/// Parses a base58 address, or returns a 400 naming what it addresses (`pool`, `mint`).
fn parse_address(raw: &str, label: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(raw)
        .map_err(|_| ApiError::BadRequest(format!("invalid {label} address: {raw}")))
}
