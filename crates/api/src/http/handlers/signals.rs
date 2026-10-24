use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Path, Query, State},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::{Stream, StreamExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::application::EnrichedSignal;
use crate::http::{
    AppState,
    cursor::encode_cursor_opt,
    dto::{
        PageResponse, SignalResponse,
        request::{EntitySignalsQuery, ListSignalsRequest, SignalsQuery, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/signals",
    params(SignalsQuery),
    responses((
        status = 200,
        description = "A page of signals.",
        body = PageResponse<SignalResponse>
    ))
)]
pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    signal_page(&state, ListSignalsRequest::parse(query)?).await
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/signals",
    params(EntitySignalsQuery),
    responses((
        status = 200,
        description = "A page of the protocol's signals.",
        body = PageResponse<SignalResponse>
    ))
)]
pub(crate) async fn list_protocol_signals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<EntitySignalsQuery>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListSignalsRequest::parse_for_entity(query, Some(protocol), None)?;
    signal_page(&state, request).await
}

#[utoipa::path(
    get,
    path = "/api/pools/{pool}/signals",
    params(EntitySignalsQuery),
    responses((
        status = 200,
        description = "A page of the pool's signals.",
        body = PageResponse<SignalResponse>
    ))
)]
pub(crate) async fn list_pool_signals(
    State(state): State<AppState>,
    Path(pool): Path<String>,
    Query(query): Query<EntitySignalsQuery>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let pool = super::known_pool(&state, &pool).await?;
    let request = ListSignalsRequest::parse_for_entity(query, None, Some(pool.pool_address))?;
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

/// Live tail of the signal feed as Server-Sent Events: new signals only, each a
/// `SignalResponse` with its id as the event id. History comes from
/// `GET /api/signals`, re-fetched on (re)connect.
///
/// ⚠️ A subscriber that lags past the broadcast capacity is dropped and its
/// EventSource reconnects: a silent gap would hide alerts.
/// ⚠️ At most `SSE_MAX_STREAMS` streams at once, then `503` with `Retry-After`:
/// each stream holds memory for as long as it lives.
#[utoipa::path(
    get,
    path = "/api/signals/stream",
    responses((
        status = 200,
        description = "Server-sent events: each `data` is a `SignalResponse`.",
        content_type = "text/event-stream",
        body = String
    ))
)]
pub(crate) async fn stream_signals(State(state): State<AppState>) -> Response {
    open_signal_stream(
        state.stream_slots.clone(),
        state.signal_stream.subscribe(),
        state.shutdown.clone(),
    )
}

/// Opens a stream if a slot is free, or refuses with `503`; without `AppState`, so a test drives the cap.
pub(crate) fn open_signal_stream(
    slots: Arc<Semaphore>,
    receiver: broadcast::Receiver<Arc<EnrichedSignal>>,
    shutdown: CancellationToken,
) -> Response {
    let Ok(slot) = slots.try_acquire_owned() else {
        return ApiError::Unavailable("too many open signal streams; retry shortly")
            .into_response();
    };
    info!("signal stream client connected");
    signal_sse(receiver, slot, shutdown).into_response()
}

/// The SSE response, without `AppState` so a test can mount it. ⚠️ It ends on the stop (`take_until`):
/// graceful shutdown waits for every connection, and this one never ends on its own. `slot`
/// is released when the stream is dropped.
pub(crate) fn signal_sse(
    receiver: broadcast::Receiver<Arc<EnrichedSignal>>,
    slot: OwnedSemaphorePermit,
    shutdown: CancellationToken,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let stream =
        futures_util::stream::unfold((receiver, slot), |(mut receiver, slot)| async move {
            match receiver.recv().await {
                // Enriched once by the poller: no database work here.
                Ok(signal) => Some((make_event(&signal), (receiver, slot))),
                // Lagged or Closed: end the stream and let the client reconnect.
                Err(_) => None,
            }
        })
        .take_until(shutdown.cancelled_owned());

    Sse::new(stream).keep_alive(
        // A comment ping, or proxies reap a connection the feed may leave quiet for long.
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

fn make_event(signal: &EnrichedSignal) -> Result<Event, axum::Error> {
    let id = signal.record.id.to_string();
    Event::default()
        .json_data(SignalResponse::from(signal.clone()))
        .map(|event| event.id(id))
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
