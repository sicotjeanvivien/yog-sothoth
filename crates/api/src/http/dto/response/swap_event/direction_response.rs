use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::TradeDirection;

/// Which token went in: `a_to_b` sold token A for token B.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum SwapEventDirectionResponse {
    AToB,
    BToA,
}

impl From<TradeDirection> for SwapEventDirectionResponse {
    fn from(direction: TradeDirection) -> Self {
        match direction {
            TradeDirection::AtoB => Self::AToB,
            TradeDirection::BtoA => Self::BToA,
        }
    }
}
