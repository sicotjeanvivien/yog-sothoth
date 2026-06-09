use crate::repositories::helper::{
    convert_i64_to_u64, convert_string_to_pubkey, convert_string_to_signature,
};
use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::MeteoraDammV2ClaimPositionFeeEvent};

/// Row shape returned by SELECTs on `position_fee_claims`. Mirrors
/// every column of the table.
#[derive(sqlx::FromRow)]
pub(super) struct MeteoraDammV2ClaimPositionFeeEventRow {
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
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
            position: convert_string_to_pubkey(row.position, "position")?,
            owner: convert_string_to_pubkey(row.owner, "owner")?,
            fee_a_claimed: convert_i64_to_u64(row.fee_a_claimed, "fee_a_claimed")?,
            fee_b_claimed: convert_i64_to_u64(row.fee_b_claimed, "fee_b_claimed")?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
