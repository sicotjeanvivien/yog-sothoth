use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// The protocol a pool, an event or a signal belongs to.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(in crate::http::dto::response) enum ProtocolResponse {
    MeteoraDammV2,
    MeteoraDlmm,
}

impl From<domain::Protocol> for ProtocolResponse {
    fn from(protocol: domain::Protocol) -> Self {
        match protocol {
            domain::Protocol::MeteoraDammV2 => Self::MeteoraDammV2,
            domain::Protocol::MeteoraDlmm => Self::MeteoraDlmm,
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_tests.rs"]
mod tests;
