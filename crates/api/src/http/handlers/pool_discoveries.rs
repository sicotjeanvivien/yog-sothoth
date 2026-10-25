//! The pools discovered in a window: first seen without their birth observed, read with the
//! TVL reading they were measured at. A discovery is not a deposit.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::Protocol;

use crate::http::{
    AppState,
    dto::{
        PoolDiscoveriesResponse, WindowedResponse,
        request::{WindowParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/pool-discoveries/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "Each protocol's pools discovered in the window, and what they hold.",
        body = WindowedResponse<PoolDiscoveriesResponse>
    ))
)]
pub(crate) async fn get_pool_discovery_totals(
    State(state): State<AppState>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolDiscoveriesResponse>>, ApiError> {
    totals(&state, params, None).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/pool-discoveries/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's pools discovered in the window, and what they hold.",
        body = WindowedResponse<PoolDiscoveriesResponse>
    ))
)]
pub(crate) async fn get_protocol_pool_discovery_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<PoolDiscoveriesResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    totals(&state, params, Some(protocol)).await
}

async fn totals(
    state: &AppState,
    params: WindowParams,
    protocol: Option<Protocol>,
) -> Result<Json<WindowedResponse<PoolDiscoveriesResponse>>, ApiError> {
    let window = params.window.into();
    let measured = state
        .measure_service
        .pool_discoveries(window, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}
