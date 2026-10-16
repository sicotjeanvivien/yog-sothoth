//! Wire shape of the readiness probe: structured, so a new check is not a breaking change.

mod checks_report_response;
mod component_status_response;
mod status_response;

use serde::Serialize;

pub(crate) use checks_report_response::ChecksReportResponse;
pub(crate) use component_status_response::ComponentStatusResponse;
pub(crate) use status_response::ReadinessStatusResponse;

/// The `/readyz` payload: the verdict, and the per-check map, present even when ready.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadinessResponse {
    pub(crate) status: ReadinessStatusResponse,
    pub(crate) checks: ChecksReportResponse,
}
