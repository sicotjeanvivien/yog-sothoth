use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::MeteoraDammV2TradeDirection;

/// Which token went in: `a_to_b` sold token A for token B.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum SwapEventDirectionResponse {
    AToB,
    BToA,
}

impl From<MeteoraDammV2TradeDirection> for SwapEventDirectionResponse {
    fn from(direction: MeteoraDammV2TradeDirection) -> Self {
        match direction {
            MeteoraDammV2TradeDirection::AtoB => Self::AToB,
            MeteoraDammV2TradeDirection::BtoA => Self::BToA,
        }
    }
}
