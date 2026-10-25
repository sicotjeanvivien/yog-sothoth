use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::LastEventKind;

/// The kind of the event that last moved the pool.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum PoolLastEventKindResponse {
    Swap,
    LiquidityAdd,
    LiquidityRemove,
}

impl From<LastEventKind> for PoolLastEventKindResponse {
    fn from(kind: LastEventKind) -> Self {
        match kind {
            LastEventKind::Swap => Self::Swap,
            LastEventKind::LiquidityAdd => Self::LiquidityAdd,
            LastEventKind::LiquidityRemove => Self::LiquidityRemove,
        }
    }
}
