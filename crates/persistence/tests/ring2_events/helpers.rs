//! Sentinels shared by every ring-2 round-trip test.

use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

pub fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}
pub fn sg() -> Signature {
    Signature::from([7u8; 64])
}
pub fn ts() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 0).unwrap()
}
