//! The claims of positions' fees and rewards, valued by yog-analytic: a protocol's totals and
//! series. Their coverage starts at the first tranche their writer valued.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use yog_core::domain::ClaimKind;

use crate::application::Measured;
use crate::http::{
    AppState,
    dto::{
        ProtocolClaimBucketResponse, ProtocolClaimTotalsResponse, SeriesResponse, WindowedResponse,
        request::{MeasureWindowParams, SeriesParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/position-fee-claims/totals",
    params(MeasureWindowParams),
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
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<WindowedResponse<ProtocolClaimTotalsResponse>>, ApiError> {
    get_totals(&state, &protocol, params, ClaimKind::PositionFee).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/position-fee-claims/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "The fees the protocol's positions claimed by `interval`, valued, from the \
                       claims' coverage.",
        body = SeriesResponse<ProtocolClaimBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_position_fee_claim_series(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolClaimBucketResponse>>, ApiError> {
    list_series(&state, &protocol, params, ClaimKind::PositionFee).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/reward-claims/totals",
    params(MeasureWindowParams),
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
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<WindowedResponse<ProtocolClaimTotalsResponse>>, ApiError> {
    get_totals(&state, &protocol, params, ClaimKind::Reward).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/reward-claims/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "The rewards the protocol's positions claimed by `interval`, valued, from \
                       the claims' coverage.",
        body = SeriesResponse<ProtocolClaimBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_reward_claim_series(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolClaimBucketResponse>>, ApiError> {
    list_series(&state, &protocol, params, ClaimKind::Reward).await
}

async fn get_totals(
    state: &AppState,
    protocol: &str,
    params: MeasureWindowParams,
    kind: ClaimKind,
) -> Result<Json<WindowedResponse<ProtocolClaimTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(protocol)?;
    let window = params.window.into();
    let measured = state
        .claim_totals_service
        .get_claim_totals(kind, window, protocol)
        .await?;
    let measured = Measured {
        period: measured.period,
        items: measured
            .items
            .into_iter()
            .map(|totals| ProtocolClaimTotalsResponse::new(protocol, totals))
            .collect::<Vec<_>>(),
    };
    Ok(Json(WindowedResponse::new(window, measured)))
}

async fn list_series(
    state: &AppState,
    protocol: &str,
    params: SeriesParams,
    kind: ClaimKind,
) -> Result<Json<SeriesResponse<ProtocolClaimBucketResponse>>, ApiError> {
    let protocol = parse_protocol(protocol)?;
    let (window, interval) = params.parse()?;
    let measured = state
        .claim_totals_service
        .list_claim_series(kind, window, interval, protocol)
        .await?;
    Ok(Json(SeriesResponse::new(window, interval, measured)))
}
