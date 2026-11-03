use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::http::{
    AppState,
    cursor::encode_cursor_opt,
    dto::{
        PageResponse, SignalResponse,
        request::{ListSignalsRequest, SignalsParams, parse_pool, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/signals",
    params(SignalsParams),
    responses((
        status = 200,
        description = "A page of signals.",
        body = PageResponse<SignalResponse>
    ))
)]
pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Query(params): Query<SignalsParams>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    signal_page(&state, ListSignalsRequest::parse(params)?).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/signals",
    params(SignalsParams),
    responses((
        status = 200,
        description = "A page of the protocol's signals.",
        body = PageResponse<SignalResponse>
    ))
)]
pub(crate) async fn list_protocol_signals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<SignalsParams>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListSignalsRequest::parse_for_entity(params, Some(protocol), None)?;
    signal_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/signals",
    params(SignalsParams),
    responses((
        status = 200,
        description = "A page of the pool's signals.",
        body = PageResponse<SignalResponse>
    ))
)]
pub(crate) async fn list_pool_signals(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(params): Query<SignalsParams>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let address = parse_pool(&pool)?;
    let request = ListSignalsRequest::parse_for_entity(params, None, Some(address))?;
    super::known_pool(&state, &pool).await?;
    signal_page(&state, request).await
}

async fn signal_page(
    state: &AppState,
    request: ListSignalsRequest,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let page = state
        .signal_service
        .list_signals(request.into_params())
        .await?;

    let items: Vec<SignalResponse> = page.items.into_iter().map(SignalResponse::from).collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;
    let prev_cursor = encode_cursor_opt(page.prev_cursor.as_ref())?;

    Ok(Json(PageResponse {
        items,
        next_cursor,
        prev_cursor,
        is_first: page.is_first,
        is_last: page.is_last,
    }))
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
