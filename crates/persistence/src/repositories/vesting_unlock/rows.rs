use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryResult,
    domain::{Protocol, VestingUnlock},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_bigdecimal_to_u128, convert_optional,
    convert_string_to_pubkey,
};

/// The reading's instant and coverage beside one of its releases: the release columns are all
/// `NULL` when the page is empty.
pub(super) struct VestingUnlockRow {
    pub(super) as_of: Option<DateTime<Utc>>,
    pub(super) covered_since: Option<DateTime<Utc>>,
    pub(super) pool_address: Option<String>,
    pub(super) token_a_symbol: Option<String>,
    pub(super) token_b_symbol: Option<String>,
    pub(super) position: Option<String>,
    pub(super) unlocks_at: Option<DateTime<Utc>>,
    pub(super) liquidity: Option<BigDecimal>,
    pub(super) amount_usd: Option<BigDecimal>,
}

impl VestingUnlockRow {
    /// `None` for the row of an empty page.
    pub(super) fn into_unlock(self, protocol: Protocol) -> RepositoryResult<Option<VestingUnlock>> {
        let (Some(pool_address), Some(position), Some(unlocks_at), Some(liquidity)) = (
            self.pool_address,
            self.position,
            self.unlocks_at,
            self.liquidity,
        ) else {
            return Ok(None);
        };
        Ok(Some(VestingUnlock {
            protocol,
            pool_address: convert_string_to_pubkey(pool_address, "pool_address")?,
            token_a_symbol: self.token_a_symbol,
            token_b_symbol: self.token_b_symbol,
            position: convert_string_to_pubkey(position, "position")?,
            unlocks_at,
            liquidity: convert_bigdecimal_to_u128(liquidity, "liquidity")?,
            amount_usd: convert_optional(
                self.amount_usd,
                "amount_usd",
                convert_bigdecimal_to_decimal,
            )?,
        }))
    }
}
