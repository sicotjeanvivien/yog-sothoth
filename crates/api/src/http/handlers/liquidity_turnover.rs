//! `liquidity-turnover/totals`: how many times the liquidity deposited changed hands.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::Protocol;

use crate::http::{
    AppState,
    dto::{
        LiquidityTurnoverTotalsResponse,
        request::{MeasureWindowParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/liquidity-turnover/totals",
    params(MeasureWindowParams),
    responses((
        status = 200,
        description = "Each protocol's volume over the window divided by its last TVL reading, and \
                       every volume over every TVL.",
        body = LiquidityTurnoverTotalsResponse
    ))
)]
pub(crate) async fn get_liquidity_turnover_totals(
    State(state): State<AppState>,
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<LiquidityTurnoverTotalsResponse>, ApiError> {
    get_totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-turnover/totals",
    params(MeasureWindowParams),
    responses((
        status = 200,
        description = "The protocol's volume over the window divided by its last TVL reading.",
        body = LiquidityTurnoverTotalsResponse
    ))
)]
pub(crate) async fn get_protocol_liquidity_turnover_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<LiquidityTurnoverTotalsResponse>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    get_totals(&state, params, Some(protocol)).await
}

async fn get_totals(
    state: &AppState,
    params: MeasureWindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<LiquidityTurnoverTotalsResponse>, ApiError> {
    let window = params.window.into();
    let read = state
        .liquidity_turnover_service
        .get_totals(window, protocol)
        .await?;
    Ok(Json(LiquidityTurnoverTotalsResponse::new(
        window,
        read.measured,
        read.total,
    )))
}
