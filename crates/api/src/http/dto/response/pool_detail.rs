mod meteora_damm_v2_properties_response;
mod meteora_dlmm_properties_response;

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolProperties;

use super::pool::PoolResponse;
use crate::application::EnrichedPoolDetail;

use meteora_damm_v2_properties_response::MeteoraDammV2PropertiesResponse;
use meteora_dlmm_properties_response::MeteoraDlmmPropertiesResponse;

/// The pool detail sheet: [`PoolResponse`] flattened, plus the properties of the
/// pool's own protocol under its name. `None` when the pool has no satellite row
/// yet: discovered, not enriched by yog-context.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolDetailResponse {
    #[serde(flatten)]
    pool: PoolResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<MeteoraDammV2PropertiesResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_dlmm: Option<MeteoraDlmmPropertiesResponse>,
}

/// The one place the read path matches a pool's protocol: each protocol has its own key.
/// ⚠️ Exhaustive on purpose: a new protocol stops compilation here; a wildcard drops it silently.
impl From<EnrichedPoolDetail> for PoolDetailResponse {
    fn from(d: EnrichedPoolDetail) -> Self {
        let (meteora_damm_v2, meteora_dlmm) = match d.properties {
            Some(PoolProperties::MeteoraDammV2(p)) => (
                Some(MeteoraDammV2PropertiesResponse::build(p, d.evaluated_at)),
                None,
            ),
            Some(PoolProperties::MeteoraDlmm(p)) => {
                (None, Some(MeteoraDlmmPropertiesResponse::from(p)))
            }
            None => (None, None),
        };

        Self {
            pool: PoolResponse::from(d.pool),
            meteora_damm_v2,
            meteora_dlmm,
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_detail_tests.rs"]
mod tests;
