//! The claims of positions' fees and rewards, valued by yog-analytic: a protocol's totals. Their
//! coverage starts at the first whole hour their writer valued.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use yog_core::domain::{ClaimTotals, Protocol, Window};

use crate::application::Measured;
use crate::http::{
    AppState,
    dto::{
        ProtocolClaimTotalsResponse, WindowedResponse, request::WindowParams,
        request::parse_protocol,
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/position-fee-claims/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The fees the protocol's positions claimed over the window, valued; no row \
                       while its claims do not cover the window whole.",
        body = WindowedResponse<ProtocolClaimTotalsResponse>
    ))
)]
pub(crate) async fn get_protocol_position_fee_claim_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolClaimTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = params.window.into();
    let measured = state
        .claim_totals_service
        .get_position_fee_claim_totals(window, protocol)
        .await?;
    Ok(respond(protocol, window, measured))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/reward-claims/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The rewards the protocol's positions claimed over the window, valued; no \
                       row while its claims do not cover the window whole.",
        body = WindowedResponse<ProtocolClaimTotalsResponse>
    ))
)]
pub(crate) async fn get_protocol_reward_claim_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolClaimTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = params.window.into();
    let measured = state
        .claim_totals_service
        .get_reward_claim_totals(window, protocol)
        .await?;
    Ok(respond(protocol, window, measured))
}

fn respond(
    protocol: Protocol,
    window: Window,
    measured: Measured<Vec<ClaimTotals>>,
) -> Json<WindowedResponse<ProtocolClaimTotalsResponse>> {
    let measured = Measured {
        period: measured.period,
        items: measured
            .items
            .into_iter()
            .map(|totals| ProtocolClaimTotalsResponse::new(protocol, totals))
            .collect::<Vec<_>>(),
    };
    Json(WindowedResponse::new(window, measured))
}
