use serde::Serialize;
use utoipa::ToSchema;
use yog_core::application::decode_meteora_damm_v2_pool_fees_update;
use yog_core::domain::MeteoraDammV2PoolChange;

use super::super::meteora_damm_v2_fees::MeteoraDammV2FeeChangesResponse;

/// What a DAMM v2 pool change is, named by `kind`, and what that change says.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum PoolChangeEventMeteoraDammV2Response {
    Status(MeteoraDammV2PoolStatusResponse),
    Fees(MeteoraDammV2PoolFeesResponse),
}

/// The pool's status set, the raw byte as the program wrote it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2PoolStatusResponse {
    status: u8,
}

/// The pool's fees updated by its operator, and what changed: `null` when the bytes do not decode.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2PoolFeesResponse {
    #[schema(min_length = 1)]
    operator: String,
    fees: Option<MeteoraDammV2FeeChangesResponse>,
}

impl From<MeteoraDammV2PoolChange> for PoolChangeEventMeteoraDammV2Response {
    fn from(change: MeteoraDammV2PoolChange) -> Self {
        match change {
            MeteoraDammV2PoolChange::Status(e) => {
                Self::Status(MeteoraDammV2PoolStatusResponse { status: e.status })
            }
            MeteoraDammV2PoolChange::Fees(e) => Self::Fees(MeteoraDammV2PoolFeesResponse {
                operator: e.operator.to_string(),
                fees: decode_meteora_damm_v2_pool_fees_update(&e.params_raw)
                    .ok()
                    .map(Into::into),
            }),
        }
    }
}
