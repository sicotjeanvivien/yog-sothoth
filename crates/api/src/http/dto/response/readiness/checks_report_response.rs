use serde::Serialize;

use super::ComponentStatusResponse;

/// Per-component status. Each field is the verdict for one check.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChecksReportResponse {
    pub(crate) db: ComponentStatusResponse,
}
