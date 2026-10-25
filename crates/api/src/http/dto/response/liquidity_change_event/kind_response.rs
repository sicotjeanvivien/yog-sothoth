use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::MeteoraDammV2LiquidityEventKind;

/// Whether the change added liquidity to the pool or removed it.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum LiquidityChangeEventKindResponse {
    Add,
    Remove,
}

impl From<MeteoraDammV2LiquidityEventKind> for LiquidityChangeEventKindResponse {
    fn from(kind: MeteoraDammV2LiquidityEventKind) -> Self {
        match kind {
            MeteoraDammV2LiquidityEventKind::Add => Self::Add,
            MeteoraDammV2LiquidityEventKind::Remove => Self::Remove,
        }
    }
}
