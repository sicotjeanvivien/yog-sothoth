use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// The protocol a pool, an event or a signal belongs to.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Protocol {
    MeteoraDammV2,
    MeteoraDlmm,
}

impl From<domain::Protocol> for Protocol {
    fn from(protocol: domain::Protocol) -> Self {
        match protocol {
            domain::Protocol::MeteoraDammV2 => Self::MeteoraDammV2,
            domain::Protocol::MeteoraDlmm => Self::MeteoraDlmm,
        }
    }
}
