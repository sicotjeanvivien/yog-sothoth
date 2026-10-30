use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// What a signal's `value` and `threshold` count; a threshold has no sign, whatever the unit.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum SignalValueUnitResponse {
    SignedRatio,
    Ratio,
    Raw,
}

impl From<domain::SignalValueUnit> for SignalValueUnitResponse {
    fn from(unit: domain::SignalValueUnit) -> Self {
        match unit {
            domain::SignalValueUnit::SignedRatio => Self::SignedRatio,
            domain::SignalValueUnit::Ratio => Self::Ratio,
            domain::SignalValueUnit::Raw => Self::Raw,
        }
    }
}

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
