use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::MeteoraDammV2ClaimPositionFeeEvent};

use crate::repositories::helper::{
    convert_i32_to_u16, convert_i64_to_u32, convert_i64_to_u64, convert_optional,
    convert_string_to_pubkey, convert_string_to_signature,
};

/// A row of `meteora_damm_v2_claim_position_fee_events`, as the lists read it.
pub(super) struct MeteoraDammV2ClaimPositionFeeEventRow {
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) position: String,
    pub(super) owner: String,
    pub(super) fee_a_claimed: i64,
    pub(super) fee_b_claimed: i64,
}

impl TryFrom<MeteoraDammV2ClaimPositionFeeEventRow> for MeteoraDammV2ClaimPositionFeeEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2ClaimPositionFeeEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2ClaimPositionFeeEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            position: convert_string_to_pubkey(row.position, "position")?,
            owner: convert_string_to_pubkey(row.owner, "owner")?,
            fee_a_claimed: convert_i64_to_u64(row.fee_a_claimed, "fee_a_claimed")?,
            fee_b_claimed: convert_i64_to_u64(row.fee_b_claimed, "fee_b_claimed")?,
        })
    }
}
