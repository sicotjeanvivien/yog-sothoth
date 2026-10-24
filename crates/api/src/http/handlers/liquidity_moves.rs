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
            ListPoolLiquidityMovesRequest, PoolLiquidityMovesQuery, WindowQuery, parse_protocol,
        },
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/totals",
    params(WindowQuery),
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
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = query.window.into();
    let measured = state
        .measure_service
        .liquidity_totals(window, protocol)
        .await?;
    let measured = crate::application::Measured {
        span: measured.span,
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
    params(WindowQuery),
    responses((
        status = 200,
        description = "The protocol's liquidity created, added and removed, hour by hour.",
        body = WindowedResponse<ProtocolLiquidityMoveBarResponse>
    ))
)]
pub(crate) async fn get_protocol_liquidity_moves_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveBarResponse>>, ApiError> {
    bars(&state, &protocol, query, Cut::Hourly).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/daily",
    params(WindowQuery),
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
    Query(query): Query<WindowQuery>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveBarResponse>>, ApiError> {
    bars(&state, &protocol, query, Cut::Daily).await
}

async fn bars(
    state: &AppState,
    protocol: &str,
    query: WindowQuery,
    cut: Cut,
) -> Result<Json<WindowedResponse<ProtocolLiquidityMoveBarResponse>>, ApiError> {
    let protocol = parse_protocol(protocol)?;
    let window = query.window.into();
    let measured = state
        .measure_service
        .liquidity_bars(window, cut, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/liquidity-moves",
    params(PoolLiquidityMovesQuery),
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
    Query(query): Query<PoolLiquidityMovesQuery>,
) -> Result<Json<WindowedPageResponse<LiquidityMoveResponse>>, ApiError> {
    let request = ListPoolLiquidityMovesRequest::parse(query)?;
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
        PoolMovesRead::CursorExpired => Err(ApiError::BadRequest(
            "`cursor` belongs to a list read before the last freeze: start again without it"
                .to_string(),
        )),
    }
}
