//! Wire shape for the readiness probe response.
//!
//! Structured JSON rather than plain text so the format scales when
//! more checks are added (Helius RPC freshness, etc.) without a
//! breaking change for monitoring tooling. Today the only check is
//! the database.

mod checks_report_response;
mod component_status_response;
mod status_response;

use serde::Serialize;

pub(crate) use checks_report_response::ChecksReportResponse;
pub(crate) use component_status_response::ComponentStatusResponse;
pub(crate) use status_response::ReadinessStatusResponse;

/// The top-level payload returned by `/readyz`.
///
/// `status` is the overall verdict — "ready" or "unhealthy". The
/// per-component map is always present (even when ready) so consumers
/// don't need branching to read individual checks.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadinessResponse {
    pub(crate) status: ReadinessStatusResponse,
    pub(crate) checks: ChecksReportResponse,
}
