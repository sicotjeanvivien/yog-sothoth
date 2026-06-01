//! Wire shape for the readiness probe response.
//!
//! Structured JSON rather than plain text so the format scales when
//! more checks are added (Helius RPC freshness, etc.) without a
//! breaking change for monitoring tooling. Today the only check is
//! the database.

use serde::Serialize;

/// The top-level payload returned by `/readyz`.
///
/// `status` is the overall verdict — "ready" or "unhealthy". The
/// per-component map is always present (even when ready) so consumers
/// don't need branching to read individual checks.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadinessResponse {
    pub(crate) status: ReadinessStatus,
    pub(crate) checks: ChecksReport,
}

/// Overall verdict — the only value clients should branch on.
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReadinessStatus {
    Ready,
    Unhealthy,
}

/// Per-component status. Each field is the verdict for one check.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChecksReport {
    pub(crate) db: ComponentStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ComponentStatus {
    Up,
    Down,
}
