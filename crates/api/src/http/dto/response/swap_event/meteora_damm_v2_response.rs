use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::MeteoraDammV2SwapEvent;

use super::fee_token_response::SwapEventFeeTokenResponse;
use crate::http::dto::schema::U128String;

/// What a DAMM v2 swap says beyond every protocol's: the pool's price after it, and the fee it
/// paid, split, in the token `feeToken` names.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SwapEventMeteoraDammV2Response {
    /// Q64.64 fixed-point.
    #[schema(value_type = U128String)]
    next_sqrt_price: String,
    #[schema(value_type = U128String)]
    claiming_fee: String,
    #[schema(value_type = U128String)]
    protocol_fee: String,
    #[schema(value_type = U128String)]
    compounding_fee: String,
    #[schema(value_type = U128String)]
    referral_fee: String,
    fee_token: SwapEventFeeTokenResponse,
}

impl From<&MeteoraDammV2SwapEvent> for SwapEventMeteoraDammV2Response {
    fn from(e: &MeteoraDammV2SwapEvent) -> Self {
        Self {
            next_sqrt_price: e.next_sqrt_price.to_string(),
            claiming_fee: e.claiming_fee.to_string(),
            protocol_fee: e.protocol_fee.to_string(),
            compounding_fee: e.compounding_fee.to_string(),
            referral_fee: e.referral_fee.to_string(),
            fee_token: if e.fee_token_is_a {
                SwapEventFeeTokenResponse::A
            } else {
                SwapEventFeeTokenResponse::B
            },
        }
    }
}
