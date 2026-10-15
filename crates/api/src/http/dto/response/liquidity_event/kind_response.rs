use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::MeteoraDammV2LiquidityEventKind;

/// Whether liquidity went into the pool or out of it.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum LiquidityEventKindResponse {
    Add,
    Remove,
}

impl From<MeteoraDammV2LiquidityEventKind> for LiquidityEventKindResponse {
    fn from(kind: MeteoraDammV2LiquidityEventKind) -> Self {
        match kind {
            MeteoraDammV2LiquidityEventKind::Add => Self::Add,
            MeteoraDammV2LiquidityEventKind::Remove => Self::Remove,
        }
    }
}
