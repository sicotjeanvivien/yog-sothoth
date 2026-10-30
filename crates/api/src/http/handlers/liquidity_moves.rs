//! The valued liquidity moves: a protocol's totals and series, and the moves of a protocol or a
//! pool. Their coverage is the liquidity's, measured from a later hour than the swaps.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use yog_core::domain::{LiquidityMoveScope, Protocol};

use crate::http::{
    AppState,
    dto::{
        LiquidityMoveResponse, ProtocolLiquidityFlowsBucketResponse,
        ProtocolLiquidityFlowsResponse, SeriesResponse, WindowedPageResponse, WindowedResponse,
        request::{
            LiquidityMovesParams, ListLiquidityMovesRequest, MeasureWindowParams, SeriesParams,
            parse_pool, parse_protocol,
        },
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/totals",
    params(MeasureWindowParams),
    responses((
        status = 200,
        description = "The protocol's liquidity created, added and removed over the window; \
                       no row while its liquidity does not cover the window whole.",
        body = WindowedResponse<ProtocolLiquidityFlowsResponse>
    ))
)]
pub(crate) async fn get_protocol_liquidity_move_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<MeasureWindowParams>,
) -> Result<Json<WindowedResponse<ProtocolLiquidityFlowsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = params.window.into();
    let measured = state
        .liquidity_move_service
        .get_liquidity_totals(window, protocol)
        .await?;
    let measured = crate::application::Measured {
        period: measured.period,
        items: measured
            .items
            .into_iter()
            .map(|totals| ProtocolLiquidityFlowsResponse::new(protocol, totals))
            .collect::<Vec<_>>(),
    };
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves/series",
    params(SeriesParams),
    responses((
        status = 200,
        description = "The protocol's liquidity created, added and removed, by `interval`, the \
                       window's own step by default: 5 minutes over `1h`, an hour over `24h`, 6 \
                       hours over `7d`, a day over `30d`.",
        body = SeriesResponse<ProtocolLiquidityFlowsBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_liquidity_moves_series(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<SeriesParams>,
) -> Result<Json<SeriesResponse<ProtocolLiquidityFlowsBucketResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let (window, interval) = params.parse()?;
    let measured = state
        .liquidity_move_service
        .list_liquidity_series(window, interval, protocol)
        .await?;
    Ok(Json(SeriesResponse::new(window, interval, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/liquidity-moves",
    params(LiquidityMovesParams),
    responses((
        status = 200,
        description = "A page of the protocol's valued liquidity moves, in the order asked; \
                       empty while its liquidity does not cover the window whole.",
        body = WindowedPageResponse<LiquidityMoveResponse>
    ))
)]
pub(crate) async fn list_protocol_liquidity_moves(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<LiquidityMovesParams>,
) -> Result<Json<WindowedPageResponse<LiquidityMoveResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let scope = LiquidityMoveScope::Protocol(protocol);
    let request = ListLiquidityMovesRequest::parse(params, scope)?;
    read_moves(&state, request, protocol).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/liquidity-moves",
    params(LiquidityMovesParams),
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
    Query(params): Query<LiquidityMovesParams>,
) -> Result<Json<WindowedPageResponse<LiquidityMoveResponse>>, ApiError> {
    let scope = LiquidityMoveScope::Pool(parse_pool(&pool)?);
    let request = ListLiquidityMovesRequest::parse(params, scope)?;
    let pool = super::known_pool(&state, &pool).await?;
    read_moves(&state, request, pool.protocol).await
}

async fn read_moves(
    state: &AppState,
    request: ListLiquidityMovesRequest,
    protocol: Protocol,
) -> Result<Json<WindowedPageResponse<LiquidityMoveResponse>>, ApiError> {
    let measured = state
        .liquidity_move_service
        .list_liquidity_moves(
            request.window,
            request.list,
            protocol,
            request.order,
            request.cursor,
            request.limit,
        )
        .await?;
    Ok(Json(WindowedPageResponse::new(request.window, measured)?))
}

#[cfg(test)]
#[path = "liquidity_moves_tests.rs"]
mod tests;
