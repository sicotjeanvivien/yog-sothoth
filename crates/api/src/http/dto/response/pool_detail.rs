mod meteora_damm_v2_properties_response;
mod meteora_dlmm_properties_response;

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolProperties;

use super::pool::PoolResponse;
use crate::application::EnrichedPoolDetail;

use meteora_damm_v2_properties_response::MeteoraDammV2PropertiesResponse;
use meteora_dlmm_properties_response::MeteoraDlmmPropertiesResponse;

/// The pool detail sheet: everything in [`PoolResponse`], plus the properties
/// that only exist for the pool's own protocol.
///
/// Flattened on the wire, so the shared fields stay at the top level exactly as
/// on the list — a client can parse a detail payload with the list schema and
/// simply ignore the extra block.
///
/// The protocol block is named after its protocol rather than tagged, so adding
/// DLMM later means adding a sibling field (`meteoraDlmm`) and no change to the
/// existing one. `None` when the pool has no satellite row yet: discovered, but
/// neither enriched by yog-context nor seen at genesis.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolDetailResponse {
    #[serde(flatten)]
    pub(crate) pool: PoolResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub(crate) meteora_damm_v2: Option<MeteoraDammV2PropertiesResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub(crate) meteora_dlmm: Option<MeteoraDlmmPropertiesResponse>,
}

/// The one place a pool's protocol is matched on the read path.
///
/// It belongs here and nowhere upstream: each protocol's properties have their
/// own wire shape under their own key, so the response type is *irreducibly*
/// per-protocol, while everything before it (`PoolService`, `EnrichedPoolDetail`)
/// only ever needs "this pool's properties, whatever they are".
///
/// The exhaustive `match` is deliberate and load-bearing: adding a protocol
/// stops compilation right here — the reminder to give it its own response
/// field. A wildcard arm would instead drop it from the wire in silence. This is
/// what happened when DLMM landed: the previously irrefutable
/// `|PoolProperties::MeteoraDammV2(p)|` closure stopped compiling, which is
/// exactly the intended behaviour.
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
