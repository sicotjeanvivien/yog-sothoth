//! Why a check failed, and what it says: the text that goes to `/fail` and
//! to the logs.

use chrono::{DateTime, Duration as ChronoDuration, Utc};

/// Why a check failed. Every variant has a reason to give — there is no
/// failure without one to put in `/fail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// These aggregates have a row waiting beyond `limit`.
    Late {
        aggregates: Vec<LateAggregate>,
        limit: ChronoDuration,
    },
    /// The backlogs could not be read; the text says why.
    Unreadable(String),
    /// The read succeeded and reported no aggregate at all — the function no
    /// longer finds them in TimescaleDB's catalog. Not `Unreadable`: the
    /// database answered, and a connection or privilege fault would be the
    /// wrong thing to look for.
    NothingReported,
}

/// One aggregate past the limit, with what the reason says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LateAggregate {
    pub(crate) name: String,
    /// How long its oldest pending row has waited.
    pub(crate) pending_for: ChronoDuration,
    /// Where its materialisation ends, `None` if it never materialised a
    /// bucket. Not part of the rule: it tells a refresh that stopped at some
    /// hour from one that never ran — which the wait alone does not.
    pub(crate) watermark: Option<DateTime<Utc>>,
}

impl LateAggregate {
    /// `swaps_hourly pending for 5h12m, materialised up to 2026-01-15 06:00 UTC`.
    fn describe(&self) -> String {
        let materialised = match self.watermark {
            Some(watermark) => format!(
                "materialised up to {}",
                watermark.format("%Y-%m-%d %H:%M UTC")
            ),
            None => "never materialised".to_string(),
        };
        format!(
            "{} pending for {}, {materialised}",
            self.name,
            hours_minutes(self.pending_for)
        )
    }
}

impl Failure {
    /// What the failure says, in the check's log and in ours.
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Late { aggregates, limit } => {
                let names = aggregates
                    .iter()
                    .map(LateAggregate::describe)
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("late (limit {}): {names}", hours_minutes(*limit))
            }
            Self::Unreadable(error) => format!("unreadable: {error}"),
            Self::NothingReported => "nothing reported: no continuous aggregate found".to_string(),
        }
    }
}

/// A duration as `4h05m`, the form every reason and log line uses.
pub(crate) fn hours_minutes(duration: ChronoDuration) -> String {
    let minutes = duration.num_minutes();
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}
