use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::TokenActivity;
use crate::domain::Window;
use crate::tools::Page;

/// A page of the ranking written at `last_hour`, the hour it ends at.
#[derive(Debug, Clone)]
pub struct TokenActivityPage {
    pub last_hour: DateTime<Utc>,
    pub page: Page<TokenActivity>,
}

/// The last item of a page in the volume ordering, `(volume_usd, mint)`, in
/// the ranking of `window` at `last_hour`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenActivityCursor {
    pub window: Window,
    pub last_hour: DateTime<Utc>,
    pub volume_usd: Decimal,
    pub mint: Pubkey,
}
