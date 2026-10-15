use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// Which way a swap went: token A sold for B, or B for A.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub(crate) enum TradeDirectionResponse {
    #[serde(rename = "a_to_b")]
    AToB,
    #[serde(rename = "b_to_a")]
    BToA,
}

impl From<domain::TradeDirection> for TradeDirectionResponse {
    fn from(direction: domain::TradeDirection) -> Self {
        match direction {
            domain::TradeDirection::AtoB => Self::AToB,
            domain::TradeDirection::BtoA => Self::BToA,
        }
    }
}
