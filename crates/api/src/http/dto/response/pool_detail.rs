//! `GET /api/pools/{pool}`: the pool of [`PoolResponse`], and the properties of its own
//! protocol under the protocol's name.

mod meteora_damm_v2_properties_response;

use serde::Serialize;
use utoipa::ToSchema;

use super::pool::PoolResponse;
use crate::application::{PoolDetail, PoolProperties};

use meteora_damm_v2_properties_response::MeteoraDammV2PropertiesResponse;

/// The block is absent until yog-context first resolves the pool's account.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolDetailResponse {
    #[serde(flatten)]
    pool: PoolResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<MeteoraDammV2PropertiesResponse>,
}

impl From<PoolDetail> for PoolDetailResponse {
    fn from(detail: PoolDetail) -> Self {
        let meteora_damm_v2 = detail.properties.map(|properties| match properties {
            PoolProperties::MeteoraDammV2(p) => {
                MeteoraDammV2PropertiesResponse::build(p, detail.read_at)
            }
        });
        Self {
            pool: detail.entry.into(),
            meteora_damm_v2,
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_detail_tests.rs"]
mod tests;
