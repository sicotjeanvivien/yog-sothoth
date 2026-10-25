use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2UpdatePoolFeesEvent},
};

use crate::repositories::helper::{
    convert_i16_to_u8, convert_i32_to_u16, convert_i64_to_u32, convert_i64_to_u64,
    convert_optional, convert_string_to_pubkey, convert_string_to_signature,
};

/// A row of `meteora_damm_v2_set_pool_status_events`, as the list reads it.
pub(super) struct MeteoraDammV2SetPoolStatusEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) status: i16,
}

impl TryFrom<MeteoraDammV2SetPoolStatusEventRow> for MeteoraDammV2SetPoolStatusEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2SetPoolStatusEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2SetPoolStatusEvent {
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
            status: convert_i16_to_u8(row.status, "status")?,
        })
    }
}

/// A row of `meteora_damm_v2_update_pool_fees_events`, as the list reads it.
pub(super) struct MeteoraDammV2UpdatePoolFeesEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) operator: String,
    pub(super) params_raw: Vec<u8>,
}

impl TryFrom<MeteoraDammV2UpdatePoolFeesEventRow> for MeteoraDammV2UpdatePoolFeesEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2UpdatePoolFeesEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2UpdatePoolFeesEvent {
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
            operator: convert_string_to_pubkey(row.operator, "operator")?,
            params_raw: row.params_raw,
        })
    }
}
