use serde::Serialize;
use utoipa::ToSchema;
use yog_core::application::decode_meteora_damm_v2_pool_fees;
use yog_core::domain::MeteoraDammV2InitializePoolEvent;

use super::super::meteora_damm_v2_fees::MeteoraDammV2FeeParametersResponse;
use crate::http::dto::schema::U128String;

/// What a DAMM v2 creation says beyond every protocol's: who paid, the price range and the
/// first price (Q64.64), the liquidity, the activation, the fee and pool modes, the token
/// flags, the amounts deposited — in raw units, the totals including the fees taken — and its
/// fees, `null` when their bytes do not decode.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PoolCreationEventMeteoraDammV2Response {
    #[schema(min_length = 1)]
    payer: String,
    #[schema(min_length = 1)]
    alpha_vault: String,
    #[schema(value_type = U128String)]
    sqrt_min_price: String,
    #[schema(value_type = U128String)]
    sqrt_max_price: String,
    #[schema(value_type = U128String)]
    sqrt_price: String,
    #[schema(value_type = U128String)]
    liquidity: String,
    activation_type: u8,
    #[schema(value_type = U128String)]
    activation_point: String,
    collect_fee_mode: u8,
    pool_type: u8,
    token_a_flag: u8,
    token_b_flag: u8,
    #[schema(value_type = U128String)]
    token_a_amount: String,
    #[schema(value_type = U128String)]
    token_b_amount: String,
    #[schema(value_type = U128String)]
    total_amount_a: String,
    #[schema(value_type = U128String)]
    total_amount_b: String,
    fees: Option<MeteoraDammV2FeeParametersResponse>,
}

impl From<&MeteoraDammV2InitializePoolEvent> for PoolCreationEventMeteoraDammV2Response {
    fn from(e: &MeteoraDammV2InitializePoolEvent) -> Self {
        Self {
            payer: e.payer.to_string(),
            alpha_vault: e.alpha_vault.to_string(),
            sqrt_min_price: e.sqrt_min_price.to_string(),
            sqrt_max_price: e.sqrt_max_price.to_string(),
            sqrt_price: e.sqrt_price.to_string(),
            liquidity: e.liquidity.to_string(),
            activation_type: e.activation_type,
            activation_point: e.activation_point.to_string(),
            collect_fee_mode: e.collect_fee_mode,
            pool_type: e.pool_type,
            token_a_flag: e.token_a_flag,
            token_b_flag: e.token_b_flag,
            token_a_amount: e.token_a_amount.to_string(),
            token_b_amount: e.token_b_amount.to_string(),
            total_amount_a: e.total_amount_a.to_string(),
            total_amount_b: e.total_amount_b.to_string(),
            fees: decode_meteora_damm_v2_pool_fees(&e.pool_fees_raw)
                .ok()
                .map(Into::into),
        }
    }
}
