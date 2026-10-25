use serde::Serialize;
use utoipa::ToSchema;

use crate::http::dto::schema::U128String;

/// What a DAMM v2 liquidity change says beyond every protocol's: the liquidity it moved.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiquidityChangeEventMeteoraDammV2Response {
    #[schema(value_type = U128String)]
    pub(super) liquidity_delta: String,
}
