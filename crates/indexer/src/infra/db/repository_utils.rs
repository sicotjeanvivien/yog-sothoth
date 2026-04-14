use std::str::FromStr;

use solana_pubkey::Pubkey;
use sqlx::types::BigDecimal;
use yog_core::{domain::Protocol, CoreError};

pub(crate) fn convert_string_to_pubkey(key: String, field: &str) -> Result<Pubkey, CoreError> {
    Pubkey::from_str(&key).map_err(|e| CoreError::ParseError {
        signature: String::new(),
        reason: format!("invalid {field} pubkey: {e}"),
    })
}

pub(crate) fn convert_u64_to_i64(v: u64, field: &str) -> Result<i64, CoreError> {
    i64::try_from(v).map_err(|e| CoreError::ParseError {
        signature: String::new(),
        reason: format!("invalid {field}: {e}"),
    })
}

pub(crate) fn convert_i64_to_u64(v: i64, field: &str) -> Result<u64, CoreError> {
    u64::try_from(v).map_err(|e| CoreError::ParseError {
        signature: String::new(),
        reason: format!("invalid {field}: {e}"),
    })
}

pub(crate) fn convert_bigdecimal_to_u128(
    bigdecimal: BigDecimal,
    field: &str,
) -> Result<u128, CoreError> {
    bigdecimal
        .to_string()
        .parse::<u128>()
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("{field} parse error: {e}"),
        })
}

pub(crate) fn parse_string_to_protocol(
    protocol: String,
    field: &str,
) -> Result<Protocol, CoreError> {
    protocol
        .parse::<Protocol>()
        .map_err(|_| CoreError::ParseError {
            signature: String::new(),
            reason: format!("unknown {field}: {}", protocol),
        })
}
