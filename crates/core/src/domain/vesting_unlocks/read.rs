//! What the lens returns: a page of the last reading, with the reading's own facts.

use chrono::{DateTime, Utc};

use super::VestingUnlock;
use crate::tools::Page;

/// A page of the releases after `as_of`, the instant of the reading they were written at.
#[derive(Debug, Clone)]
pub struct VestingUnlockPage {
    pub as_of: DateTime<Utc>,
    /// The first vesting observed: those locked before it are not listed.
    pub covered_since: Option<DateTime<Utc>>,
    pub page: Page<VestingUnlock>,
}
