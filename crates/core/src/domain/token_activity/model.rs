//! What each token was traded for over a window, from yog-analytic's
//! aggregate by token: the volume of the pools that hold it.

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::TokenMetadata;

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
