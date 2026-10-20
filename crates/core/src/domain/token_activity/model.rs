//! What each token was traded for over a window, from yog-analytic's
//! aggregate by token: the volume of the pools that hold it.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::TokenMetadata;
use crate::tools::Page;

/// One token over a window. ⚠️ A swap counts for both its tokens: the volumes
/// of all tokens add up to twice the total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenActivity {
    pub mint: Pubkey,
    /// `None` until yog-context resolves the mint.
    pub metadata: Option<TokenMetadata>,
    /// The valued pool-hours only, zero when none is.
    pub volume_usd: Decimal,
    pub pools: i64,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
}

/// A page of the ranking written at `last_hour`, the hour it ends at.
#[derive(Debug, Clone)]
pub struct TokenActivityPage {
    pub last_hour: DateTime<Utc>,
    pub page: Page<TokenActivity>,
}

/// The last item of a page in the volume ordering, `(volume_usd, mint)`, in
/// the ranking of `last_hour`: each freeze rewrites the ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenActivityCursor {
    pub last_hour: DateTime<Utc>,
    pub volume_usd: Decimal,
    pub mint: Pubkey,
}
