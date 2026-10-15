use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// The kind of the event that last changed the pool's state.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum LastEventKindResponse {
    Swap,
    LiquidityAdd,
    LiquidityRemove,
}

impl From<domain::LastEventKind> for LastEventKindResponse {
    fn from(kind: domain::LastEventKind) -> Self {
        match kind {
            domain::LastEventKind::Swap => Self::Swap,
            domain::LastEventKind::LiquidityAdd => Self::LiquidityAdd,
            domain::LastEventKind::LiquidityRemove => Self::LiquidityRemove,
        }
    }
}
