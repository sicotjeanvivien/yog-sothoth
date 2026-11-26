/// What an idempotent insert did.
/// ⚠️ Under `ON CONFLICT DO NOTHING`, `Ok` covers a dropped row too: discarding
/// this outcome hides a unique key too narrow to tell two events apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a skipped insert means an event was dropped — count it"]
pub enum InsertOutcome {
    /// The row was written.
    Inserted,
    /// The unique key was taken: expected on a replay, suspicious in a live stream.
    Skipped,
}

impl InsertOutcome {
    /// From the rows affected by an `INSERT … ON CONFLICT DO NOTHING`.
    pub fn from_rows_affected(rows: u64) -> Self {
        if rows == 0 {
            Self::Skipped
        } else {
            Self::Inserted
        }
    }

    pub fn is_skipped(self) -> bool {
        matches!(self, Self::Skipped)
    }
}
