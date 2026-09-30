//! Why a check failed, and what it says: the text that goes to `/fail` and
//! to the logs.

use chrono::Duration as ChronoDuration;

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
    /// The read succeeded and reported no aggregate at all — the function no
    /// longer finds them in TimescaleDB's catalog. Not `Unreadable`: the
    /// database answered, and a connection or privilege fault would be the
    /// wrong thing to look for.
    NothingReported,
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
            Self::NothingReported => "nothing reported: no continuous aggregate found".to_string(),
        }
    }
}

/// A duration as `4h05m`, the form every reason and log line uses.
pub(crate) fn hours_minutes(duration: ChronoDuration) -> String {
    let minutes = duration.num_minutes();
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}
