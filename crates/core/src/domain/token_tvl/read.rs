//! The contract of the lens's read: the order and cursor it takes, the page it returns.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::TokenTvl;
use crate::domain::Protocol;
use crate::tools::Page;

/// The order of the ranking, each descending then by mint; an unknown locked part last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenTvlOrder {
    #[default]
    Tvl,
    Locked,
}

/// The last token of a page, in the ranking of `protocol`'s reading at `at`, in `order`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenTvlCursor {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub order: TokenTvlOrder,
    /// The order's value, `None` for an unknown locked part.
    pub value: Option<Decimal>,
    pub mint: Pubkey,
}

/// The protocol's own reading, the tokens' denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenTvlTotal {
    pub tvl_usd: Option<Decimal>,
    pub locked_usd: Option<Decimal>,
}

/// A page of the ranking of the reading at `at`, the close of its hour.
#[derive(Debug, Clone)]
pub struct TokenTvlPage {
    pub at: DateTime<Utc>,
    /// The first vesting observed: the locked parts leave out those locked before it.
    pub covered_since: Option<DateTime<Utc>>,
    pub total: TokenTvlTotal,
    pub page: Page<TokenTvl>,
}
