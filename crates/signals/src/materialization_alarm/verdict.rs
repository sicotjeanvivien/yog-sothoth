//! How one check of the backlogs ends, and what it says when it fails.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use yog_core::domain::MaterializationBacklog;

/// How one check ended — and what the heartbeat is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// No raw row has waited longer than the limit.
    OnTime,
    /// Something is wrong, and [`Failure::reason`] says what.
    Failed(Failure),
}

/// Why a check failed. Every variant has a reason to give — there is no
/// failure without one to put in `/fail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// These aggregates have a row waiting beyond `limit`, with how long.
    Late {
        aggregates: Vec<(String, ChronoDuration)>,
        limit: ChronoDuration,
    },
    /// The backlogs could not be read; the text says why.
    Unreadable(String),
}

impl Verdict {
    /// Judge what the database reported, at `now`.
    pub(crate) fn judge(
        backlogs: &[MaterializationBacklog],
        now: DateTime<Utc>,
        max_wait: ChronoDuration,
    ) -> Self {
        // ⚠️ **Nothing reported is not "on time".** The function finds the
        // aggregates in TimescaleDB's catalog; if an upgrade changed what it
        // joins on, it would return no row, and every check would ping success
        // while watching nothing — the silent green this alarm exists to end.
        // The schema always holds aggregates, so an empty answer is a fault.
        if backlogs.is_empty() {
            return Self::Failed(Failure::Unreadable(
                "no continuous aggregate reported".to_string(),
            ));
        }
        let aggregates: Vec<_> = backlogs
            .iter()
            .filter_map(|b| Some((b.aggregate.clone(), b.late_by(now, max_wait)?)))
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
        }
    }

    /// What must change for the logs to speak again: the outcome, or which
    /// aggregates are late — not how long they have been.
    pub(super) fn state(&self) -> (&'static str, Vec<&str>) {
        let names = match self {
            Self::Failed(Failure::Late { aggregates, .. }) => {
                aggregates.iter().map(|(name, _)| name.as_str()).collect()
            }
            _ => Vec::new(),
        };
        (self.label(), names)
    }
}

impl Failure {
    /// What the failure says, in the check's log and in ours.
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Late { aggregates, limit } => {
                let names = aggregates
                    .iter()
                    .map(|(name, wait)| format!("{name} pending for {}", hours_minutes(*wait)))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("late (limit {}): {names}", hours_minutes(*limit))
            }
            Self::Unreadable(error) => format!("unreadable: {error}"),
        }
    }
}

pub(super) fn hours_minutes(duration: ChronoDuration) -> String {
    let minutes = duration.num_minutes();
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}
