/// What an idempotent insert actually did.
///
/// Event inserts are `ON CONFLICT … DO NOTHING`: re-ingesting a transaction
/// must not fail, and must not duplicate. That makes "no error" ambiguous —
/// it covers both a row written and a row silently dropped.
///
/// Returning which one happened is what makes the drop countable. Discarding
/// it is how a unique key too narrow to tell two events apart went unnoticed
/// for months: every hop of a routed transaction after the first conflicted,
/// `DO NOTHING` swallowed it, and `Ok(())` said everything was fine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a skipped insert means an event was dropped — count it"]
pub enum InsertOutcome {
    /// The row was written.
    Inserted,
    /// A row with the same unique key was already there, so nothing was
    /// written. Expected on a replay; suspicious in a live stream.
    Skipped,
}

impl InsertOutcome {
    /// Build from what Postgres reported for an `INSERT … ON CONFLICT DO
    /// NOTHING`: one row affected means written, zero means the conflict
    /// target matched.
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
