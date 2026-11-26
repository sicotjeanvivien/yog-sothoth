use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use super::TokenMetadata;
use crate::domain::TokenPrice;

/// A token of the catalog: its metadata and its latest price, `None` before the first.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenEntry {
    pub metadata: TokenMetadata,
    pub price: Option<TokenPrice>,
}

/// The key of the last token of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenCursor {
    pub fetched_at: DateTime<Utc>,
    pub mint: Pubkey,
}
