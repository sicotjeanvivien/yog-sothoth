//! What the lens returns: a page of the last reading, with the reading's own facts.

use chrono::{DateTime, Utc};

use super::{TokenTvl, TokenTvlTotal};
use crate::tools::Page;

/// A page of the ranking of the reading at `at`, the close of its hour.
#[derive(Debug, Clone)]
pub struct TokenTvlPage {
    pub at: DateTime<Utc>,
    /// The first vesting observed: the locked parts leave out those locked before it.
    pub covered_since: Option<DateTime<Utc>>,
    pub total: TokenTvlTotal,
    pub page: Page<TokenTvl>,
}
