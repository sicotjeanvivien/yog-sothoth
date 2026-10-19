//! Request DTO for `GET /api/tokens/{mint}`.

use solana_pubkey::Pubkey;

use crate::http::error::ApiError;

#[derive(Debug)]
pub(crate) struct GetTokenRequest {
    pub(crate) mint: Pubkey,
}

impl GetTokenRequest {
    pub(crate) fn parse(mint: String) -> Result<Self, ApiError> {
        Ok(Self {
            mint: super::parse_address(&mint, "mint")?,
        })
    }
}

#[cfg(test)]
#[path = "tests/get_token_tests.rs"]
mod tests;
