//! How one check of the backlogs ends.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use yog_core::domain::MaterializationBacklog;

use super::failure::{Failure, LateAggregate};

/// How one check ended — and what the heartbeat is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// No raw row has waited longer than the limit.
    OnTime,
    /// Something is wrong, and [`Failure::reason`] says what.
    Failed(Failure),
}

impl Verdict {
    /// Judge what the database reported, at `now`.
    pub(crate) fn judge(
        backlogs: &[MaterializationBacklog],
        now: DateTime<Utc>,
        max_wait: ChronoDuration,
    ) -> Self {
        // ⚠️ Nothing reported is not "on time": an upgrade changing what the catalog query joins on
        // would return no row, and every check would ping success while watching nothing.
        if backlogs.is_empty() {
            return Self::Failed(Failure::NothingReported);
        }
        let aggregates: Vec<_> = backlogs
            .iter()
            .filter_map(|b| {
                Some(LateAggregate {
                    name: b.aggregate.clone(),
                    pending_for: b.late_by(now, max_wait)?,
                    watermark: b.watermark,
                })
            })
            .collect();
        if aggregates.is_empty() {
            Self::OnTime
        } else {
            Self::Failed(Failure::Late {
                aggregates,
                limit: max_wait,
            })
        }
    }

    /// The `outcome` label of the checks counter.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::OnTime => "on_time",
            Self::Failed(Failure::Late { .. }) => "late",
            Self::Failed(Failure::Unreadable(_)) => "unreadable",
            Self::Failed(Failure::NothingReported) => "nothing_reported",
        }
    }

    /// What must change for the logs to speak again: the outcome or the late aggregates, not their age.
    pub(crate) fn state(&self) -> (&'static str, Vec<&str>) {
        let names = match self {
            Self::Failed(Failure::Late { aggregates, .. }) => {
                aggregates.iter().map(|late| late.name.as_str()).collect()
            }
            _ => Vec::new(),
        };
        (self.label(), names)
    }
}
