//! `/api/live`: one SSE connection carries every topic a page names, for every protocol or for
//! the entity in the path. A message is named by its topic, its `data` is an item of that
//! collection, and its `id` the connection's [`LiveCursor`]. A `Last-Event-ID` is not read.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::{
    Stream, StreamExt,
    future::ready,
    stream::{self, BoxStream},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_core::domain::Protocol;

use crate::application::EnrichedSignal;
use crate::http::{
    AppState,
    cursor::LiveCursor,
    dto::{
        SignalResponse,
        request::{LiveParams, LiveRequest, LiveTopic, parse_protocol},
    },
    error::ApiError,
};

/// One item of a topic, its event still to be named and numbered by the connection.
pub(crate) struct Delivery {
    topic: LiveTopic,
    id: i64,
    event: Result<Event, axum::Error>,
}

/// A topic's items; `None` once it can no longer follow (lagged, or its source closed), which
/// ends the whole connection: a silent gap in one topic would pass for a quiet feed.
pub(crate) type TopicStream = BoxStream<'static, Option<Delivery>>;

/// Live items of every protocol.
///
/// ⚠️ At most `SSE_MAX_STREAMS` connections at once, whatever their topics, then `503`.
#[utoipa::path(
    get,
    path = "/api/live",
    params(LiveParams),
    responses((
        status = 200,
        description = "Server-sent events, each named by its topic: `signals` carries a \
                       `SignalResponse`. The `id` is an opaque cursor.",
        content_type = "text/event-stream",
        body = String
    ))
)]
pub(crate) async fn live(
    State(state): State<AppState>,
    Query(params): Query<LiveParams>,
) -> Result<Response, ApiError> {
    let request = LiveRequest::parse(params, None)?;
    Ok(open_live(
        state.stream_slots.clone(),
        || topic_streams(&state, &request),
        state.shutdown.clone(),
    ))
}

/// Live items of one protocol: the same topics, filtered by the server.
#[utoipa::path(
    get,
    path = "/api/live/protocols/{protocol}",
    params(LiveParams),
    responses((
        status = 200,
        description = "Server-sent events of the protocol, each named by its topic: `signals` \
                       carries a `SignalResponse`. The `id` is an opaque cursor.",
        content_type = "text/event-stream",
        body = String
    ))
)]
pub(crate) async fn protocol_live(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<LiveParams>,
) -> Result<Response, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = LiveRequest::parse(params, Some(protocol))?;
    Ok(open_live(
        state.stream_slots.clone(),
        || topic_streams(&state, &request),
        state.shutdown.clone(),
    ))
}

/// One subscription per topic asked for: a source nobody asked for keeps its poller idle.
fn topic_streams(state: &AppState, request: &LiveRequest) -> Vec<TopicStream> {
    request
        .topics
        .iter()
        .map(|topic| match topic {
            LiveTopic::Signals => signals_topic(state.live_signals.subscribe(), request.protocol),
        })
        .collect()
}

/// The signals as they are broadcast, those of `protocol` only when one is given.
pub(crate) fn signals_topic(
    receiver: broadcast::Receiver<Arc<EnrichedSignal>>,
    protocol: Option<Protocol>,
) -> TopicStream {
    stream::unfold(Some(receiver), move |state| async move {
        let mut receiver = state?;
        loop {
            match receiver.recv().await {
                Ok(signal) if protocol.is_none_or(|p| p == signal.record.signal.protocol) => {
                    let delivery = Delivery {
                        topic: LiveTopic::Signals,
                        id: signal.record.id,
                        // Enriched once by the poller: no database work here.
                        event: Event::default()
                            .json_data(SignalResponse::from(EnrichedSignal::clone(&signal))),
                    };
                    return Some((Some(delivery), Some(receiver)));
                }
                Ok(_) => {}
                Err(_) => return Some((None, None)),
            }
        }
    })
    .boxed()
}

/// Opens a connection if a slot is free, or refuses with `503`; without `AppState`, so a test
/// drives the cap. ⚠️ Subscribed only once the slot is held: a refused client would otherwise
/// count as a listener, and wake an idle poller.
pub(crate) fn open_live(
    slots: Arc<Semaphore>,
    subscribe: impl FnOnce() -> Vec<TopicStream>,
    shutdown: CancellationToken,
) -> Response {
    let Ok(slot) = slots.try_acquire_owned() else {
        return ApiError::Unavailable("too many open live connections; retry shortly")
            .into_response();
    };
    let topics = subscribe();
    info!(topics = topics.len(), "live client connected");
    live_sse(topics, slot, shutdown).into_response()
}

/// The SSE response. ⚠️ It ends on the stop (`take_until`): graceful shutdown waits for every
/// connection, and this one never ends on its own. `slot` is released when the stream is dropped.
pub(crate) fn live_sse(
    topics: Vec<TopicStream>,
    slot: OwnedSemaphorePermit,
    shutdown: CancellationToken,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let stream = stream::select_all(topics)
        .take_while(|delivery| ready(delivery.is_some()))
        .filter_map(ready)
        .scan(
            (LiveCursor::default(), slot),
            |(cursor, _slot), delivery| {
                cursor.record(delivery.topic, delivery.id);
                let event = cursor.encode().map_err(axum::Error::new).and_then(|id| {
                    delivery
                        .event
                        .map(|event| event.event(delivery.topic.name()).id(id))
                });
                ready(Some(event))
            },
        )
        .take_until(shutdown.cancelled_owned());

    Sse::new(stream).keep_alive(
        // A comment ping, or proxies reap a connection the feed may leave quiet for long.
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
