use serde::Serialize;
use utoipa::ToSchema;

/// The token a DAMM v2 swap paid its fee in.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum SwapEventFeeTokenResponse {
    A,
    B,
}
