use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain;

/// How serious a signal is.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SeverityResponse {
    Info,
    Warning,
    Critical,
}

impl From<domain::Severity> for SeverityResponse {
    fn from(severity: domain::Severity) -> Self {
        match severity {
            domain::Severity::Info => Self::Info,
            domain::Severity::Warning => Self::Warning,
            domain::Severity::Critical => Self::Critical,
        }
    }
}
