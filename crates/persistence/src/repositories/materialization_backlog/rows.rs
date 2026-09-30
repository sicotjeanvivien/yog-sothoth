use chrono::{DateTime, Utc};
use yog_core::domain::MaterializationBacklog;

/// Row shape of `yog_cagg_materialization_backlog()`. Both timestamps are
/// nullable by design: no watermark before a first bucket, no pending row when
/// nothing waits.
#[derive(sqlx::FromRow)]
pub(super) struct MaterializationBacklogRow {
    pub(super) aggregate: String,
    pub(super) watermark: Option<DateTime<Utc>>,
    pub(super) oldest_pending_at: Option<DateTime<Utc>>,
}

/// `From`, not `TryFrom`: every column already has its domain type — a view
/// name and two timestamps — so there is nothing to parse and nothing to
/// refuse.
impl From<MaterializationBacklogRow> for MaterializationBacklog {
    fn from(row: MaterializationBacklogRow) -> Self {
        MaterializationBacklog {
            aggregate: row.aggregate,
            watermark: row.watermark,
            oldest_pending_at: row.oldest_pending_at,
        }
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
