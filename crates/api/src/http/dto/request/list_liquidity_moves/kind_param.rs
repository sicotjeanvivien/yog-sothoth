use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::LiquidityMoveKind;

/// `create`, the pools' creations, `add` or `remove`: the moves of that kind only.
#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KindParam {
    Create,
    Add,
    Remove,
}

impl From<KindParam> for LiquidityMoveKind {
    fn from(value: KindParam) -> Self {
        match value {
            KindParam::Create => LiquidityMoveKind::Create,
            KindParam::Add => LiquidityMoveKind::Add,
            KindParam::Remove => LiquidityMoveKind::Remove,
        }
    }
}
