use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// One token of a pool's pair.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum PairTokenResponse {
    TokenA,
    TokenB,
}

impl From<domain::PairToken> for PairTokenResponse {
    fn from(token: domain::PairToken) -> Self {
        match token {
            domain::PairToken::TokenA => Self::TokenA,
            domain::PairToken::TokenB => Self::TokenB,
        }
    }
}
