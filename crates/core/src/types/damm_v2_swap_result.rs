use chrono::{DateTime, Utc};

/// Raw data extracted from a DAMM v2 swap transaction.
#[derive(Debug, PartialEq)]
pub struct DammV2SwapResult {
    pub pool_address: String,
    pub token_in_mint: String,
    pub token_out_mint: String,
    pub amount_in: u64,
    pub amount_out: u64,
    pub reserve_a_before: u64,
    pub reserve_b_before: u64,
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}
