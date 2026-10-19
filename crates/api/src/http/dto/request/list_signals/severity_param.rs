use serde::Deserialize;
use utoipa::ToSchema;
use yog_core::domain::Severity;

/// Wire form of the severity filter. An unknown value fails serde
/// deserialization → axum returns 400 before the handler runs.
#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum SeverityParam {
    Info,
    Warning,
    Critical,
}

impl From<SeverityParam> for Severity {
    fn from(value: SeverityParam) -> Self {
        match value {
            SeverityParam::Info => Severity::Info,
            SeverityParam::Warning => Severity::Warning,
            SeverityParam::Critical => Severity::Critical,
        }
    }
}
