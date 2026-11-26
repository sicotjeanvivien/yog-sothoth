use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

/// The key of the last price of a page, and the list it was read from — its mint and the
/// start of its window, fixed at its first page: a page of another list would not follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenPriceCursor {
    pub mint: Pubkey,
    pub from: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
}
