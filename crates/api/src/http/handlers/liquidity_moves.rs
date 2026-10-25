//! The valued liquidity moves: a protocol's totals and series, and a pool's moves. Their
//! coverage is the liquidity's, measured from a later hour than the swaps.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::application::{Cut, PoolMovesRead};
use crate::http::{
    AppState,
    dto::{
        LiquidityMoveResponse, ProtocolLiquidityMoveBarResponse,
        ProtocolLiquidityMoveTotalsResponse, WindowedPageResponse, WindowedResponse,
        request::{
            ListPoolLiquidityMovesRequest, PoolLiquidityMovesParams, WindowParams, parse_protocol,
        },
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's liquidity created, added and removed over the window; \
                       no row while its liquidity does not cover the window whole.",
        body = WindowedResponse<ProtocolLiquidityMoveTotalsResponse>
    ))
)]
pub(crate) async fn get_protocol_liquidity_move_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = params.window.into();
    let measured = state
        .measure_service
        .liquidity_totals(window, protocol)
        .await?;
    let measured = crate::application::Measured {
        period: measured.period,
        items: measured
            .items
            .into_iter()
            .map(|sums| ProtocolLiquidityMoveTotalsResponse::new(protocol, sums))
            .collect::<Vec<_>>(),
    };
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's liquidity created, added and removed, hour by hour.",
        body = WindowedResponse<ProtocolLiquidityMoveBarResponse>
    ))
)]
pub(crate) async fn get_protocol_liquidity_moves_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveBarResponse>>, ApiError> {
    bars(&state, &protocol, params, Cut::Hourly).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's liquidity created, added and removed, by 24 hours \
                       ending at `asOf`.",
        body = WindowedResponse<ProtocolLiquidityMoveBarResponse>
    ))
)]
pub(crate) async fn get_protocol_liquidity_moves_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveBarResponse>>, ApiError> {
    bars(&state, &protocol, params, Cut::Daily).await
}

async fn bars(
    state: &AppState,
    protocol: &str,
    params: WindowParams,
    cut: Cut,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveBarResponse>>, ApiError> {
    let protocol = parse_protocol(protocol)?;
    let window = params.window.into();
    let measured = state
        .measure_service
        .liquidity_bars(window, cut, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/liquidity-moves",
    params(PoolLiquidityMovesParams),
    responses((
        status = 200,
        description = "A page of the pool's valued liquidity moves, in the order asked; \
                       empty while its protocol's liquidity does not cover the window whole.",
        body = WindowedPageResponse<LiquidityMoveResponse>
    ))
)]
pub(crate) async fn list_pool_liquidity_moves(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<PoolLiquidityMovesParams>,
) -> Result<Json<WindowedPageResponse<LiquidityMoveResponse>>, ApiError> {
    let request = ListPoolLiquidityMovesRequest::parse(params)?;
    let pool = super::known_pool(&state, &pool).await?;
    match state
        .measure_service
        .pool_liquidity_moves(
            request.window,
            &pool,
            request.order,
            request.cursor,
            request.limit,
        )
        .await?
    {
        PoolMovesRead::Page(measured) => {
            Ok(Json(WindowedPageResponse::new(request.window, measured)?))
        }
        PoolMovesRead::CursorRefused => Err(ApiError::BadRequest(
            "`cursor` belongs to another list, or to one read before the last freeze: start \
             again without it"
                .to_string(),
        )),
    }
}
