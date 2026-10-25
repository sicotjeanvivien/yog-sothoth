//! The rotation runs: a protocol's list, totals and series, and a pool's list. The lists and
//! totals end at the last tranche written, the series at `asOf`.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use yog_core::domain::{Protocol, RotationRunScope};

use crate::application::BucketPeriod;
use crate::http::{
    AppState,
    dto::{
        ProtocolRotationRunBucketResponse, ProtocolRotationRunTotalsResponse, RotationRunResponse,
        TrancheWindowedResponse, WindowedPageResponse, WindowedResponse,
        request::{
            ListRotationRunsRequest, RotationRunsParams, WindowParams, parse_pool, parse_protocol,
        },
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/rotation-runs",
    params(RotationRunsParams),
    responses((
        status = 200,
        description = "A page of the protocol's rotation runs started in the window, in the order \
                       asked, through the last tranche written; empty while not covered.",
        body = WindowedPageResponse<RotationRunResponse>
    ))
)]
pub(crate) async fn list_protocol_rotation_runs(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<RotationRunsParams>,
) -> Result<Json<WindowedPageResponse<RotationRunResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    list_runs(
        &state,
        params,
        RotationRunScope::Protocol(protocol),
        protocol,
    )
    .await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/rotation-runs",
    params(RotationRunsParams),
    responses((
        status = 200,
        description = "A page of the pool's rotation runs started in the window, in the order \
                       asked, through the last tranche written; empty while not covered.",
        body = WindowedPageResponse<RotationRunResponse>
    ))
)]
pub(crate) async fn list_pool_rotation_runs(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<RotationRunsParams>,
) -> Result<Json<WindowedPageResponse<RotationRunResponse>>, ApiError> {
    let address = parse_pool(&pool)?;
    let scope = RotationRunScope::Pool(address);
    let request = ListRotationRunsRequest::parse(params, scope)?;
    let pool = super::known_pool(&state, &pool).await?;
    read_runs(&state, request, scope, pool.protocol).await
}

async fn list_runs(
    state: &AppState,
    params: RotationRunsParams,
    scope: RotationRunScope,
    protocol: Protocol,
) -> Result<Json<WindowedPageResponse<RotationRunResponse>>, ApiError> {
    let request = ListRotationRunsRequest::parse(params, scope)?;
    read_runs(state, request, scope, protocol).await
}

async fn read_runs(
    state: &AppState,
    request: ListRotationRunsRequest,
    scope: RotationRunScope,
    protocol: Protocol,
) -> Result<Json<WindowedPageResponse<RotationRunResponse>>, ApiError> {
    let measured = state
        .rotation_run_service
        .list_runs(
            request.window,
            scope,
            protocol,
            request.order,
            request.cursor,
            request.limit,
        )
        .await?;
    Ok(Json(WindowedPageResponse::new(request.window, measured)?))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/rotation-runs/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's rotation runs started in the window, through the last \
                       tranche written; no row while not covered.",
        body = TrancheWindowedResponse<ProtocolRotationRunTotalsResponse>
    ))
)]
pub(crate) async fn get_protocol_rotation_run_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<TrancheWindowedResponse<ProtocolRotationRunTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = params.window.into();
    let measured = state
        .rotation_run_service
        .get_totals(window, protocol)
        .await?;
    let measured = crate::application::Measured {
        period: measured.period,
        items: measured
            .items
            .into_iter()
            .map(|totals| ProtocolRotationRunTotalsResponse::new(protocol, totals))
            .collect::<Vec<_>>(),
    };
    Ok(Json(TrancheWindowedResponse::new(window, measured)))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/rotation-runs/hourly",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's volume in rotation runs, hour by hour.",
        body = WindowedResponse<ProtocolRotationRunBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_rotation_runs_hourly(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolRotationRunBucketResponse>>, ApiError> {
    list_buckets(&state, &protocol, params, BucketPeriod::Hour).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/rotation-runs/daily",
    params(WindowParams),
    responses((
        status = 200,
        description = "The protocol's volume in rotation runs, by 24 hours ending at `asOf`.",
        body = WindowedResponse<ProtocolRotationRunBucketResponse>
    ))
)]
pub(crate) async fn get_protocol_rotation_runs_daily(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolRotationRunBucketResponse>>, ApiError> {
    list_buckets(&state, &protocol, params, BucketPeriod::Day).await
}

async fn list_buckets(
    state: &AppState,
    protocol: &str,
    params: WindowParams,
    bucket: BucketPeriod,
) -> Result<Json<WindowedResponse<ProtocolRotationRunBucketResponse>>, ApiError> {
    let protocol = parse_protocol(protocol)?;
    let window = params.window.into();
    let measured = state
        .rotation_run_service
        .list_buckets(window, bucket, protocol)
        .await?;
    Ok(Json(WindowedResponse::new(window, measured)))
}

#[cfg(test)]
#[path = "rotation_runs_tests.rs"]
mod tests;
