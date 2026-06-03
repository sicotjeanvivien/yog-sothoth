use chrono::{DateTime, Utc};
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{ClaimRewardEvent, Protocol},
};

use crate::repository_utils::{
    convert_i64_to_u64, convert_string_to_pubkey, convert_string_to_signature,
};

/// Row shape returned by SELECTs on `reward_claims`. Mirrors every
/// column of the table.
///
/// `reward_index` is stored as `SMALLINT` (i16) because Postgres has
/// no native u8; the domain narrows it back to u8 via `TryFrom`,
/// surfacing out-of-range values as `Integrity`.
#[derive(sqlx::FromRow)]
pub(super) struct ClaimRewardEventRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) position: String,
    pub(super) owner: String,
    pub(super) mint_reward: String,
    pub(super) reward_index: i16,
    pub(super) total_reward: i64,
}

impl TryFrom<ClaimRewardEventRow> for ClaimRewardEvent {
    type Error = RepositoryError;

    fn try_from(row: ClaimRewardEventRow) -> Result<Self, Self::Error> {
        let reward_index = u8::try_from(row.reward_index).map_err(|_| {
            RepositoryError::Integrity(format!("invalid reward_index: {}", row.reward_index))
        })?;

        Ok(ClaimRewardEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            position: convert_string_to_pubkey(row.position, "position")?,
            owner: convert_string_to_pubkey(row.owner, "owner")?,
            mint_reward: convert_string_to_pubkey(row.mint_reward, "mint_reward")?,
            reward_index,
            total_reward: convert_i64_to_u64(row.total_reward, "total_reward")?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
