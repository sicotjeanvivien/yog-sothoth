use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::{Stream, StreamExt};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use yog_core::domain::SignalRecord;

use crate::application::{EnrichedSignal, SignalService};
use crate::bootstrap::AppState;
use crate::http::{
    cursor::encode_cursor_opt,
    dto::{PageResponse, SignalResponse, request::ListSignalsRequest},
    error::ApiError,
    query::SignalsQuery,
};

// ===========================================================================
// GET /api/signals
// ===========================================================================

pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let request = ListSignalsRequest::parse(query)?;
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

// ===========================================================================
// GET /api/signals/stream
// ===========================================================================

/// Live tail of the signal feed, as Server-Sent Events.
///
/// Each connection subscribes to the process-wide broadcast fed by the
/// [`SignalStreamPoller`]; events carry the same JSON shape as the list
/// items (`SignalResponse`), with the signal's id as the SSE event id.
/// The stream carries **new** signals only — clients load history from
/// `GET /api/signals` and re-fetch it when they (re)connect.
///
/// The stream ends when the subscriber lags past the broadcast capacity
/// (a too-slow client would otherwise silently miss alerts) — the
/// browser's EventSource then reconnects on its own.
///
/// [`SignalStreamPoller`]: crate::application::SignalStreamPoller
pub(crate) async fn stream_signals(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    info!("signal stream client connected");
    signal_sse(
        state.signal_stream.subscribe(),
        state.signal_service.clone(),
        state.shutdown.clone(),
    )
}

/// The SSE response itself, apart from the state it is read from — so that a
/// test can mount it without a database.
///
/// ⚠️ **The stream ends when the process is asked to stop, and it has to.** A
/// graceful shutdown waits for every open connection to finish, and this one
/// never finishes on its own: the `unfold` below has no end, and the keep-alive
/// keeps the connection busy. Without `take_until`, one open dashboard tab
/// would hold the stop until the grace expires. Ending the stream closes the
/// response cleanly, and the browser's EventSource reconnects on its own — to
/// the next instance, once it is up. No terminal event is sent: the client
/// has nothing to do with it that reconnecting does not already do.
pub(crate) fn signal_sse(
    receiver: broadcast::Receiver<SignalRecord>,
    service: Arc<SignalService>,
    shutdown: CancellationToken,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let stream = futures_util::stream::unfold(
        (receiver, service),
        |(mut receiver, service)| async move {
            match receiver.recv().await {
                Ok(record) => {
                    // The poller broadcasts bare records; the token pair is
                    // resolved per event, at delivery. On failure the alert
                    // still goes out, just without its pair — delivering
                    // beats decorating.
                    let enriched = match service.enrich_one(record.clone()).await {
                        Ok(enriched) => enriched,
                        Err(error) => {
                            warn!(error = %error, "signal stream: token enrichment failed — emitting bare signal");
                            EnrichedSignal::bare(record)
                        }
                    };
                    Some((make_event(enriched), (receiver, service)))
                }
                // Lagged (client too slow) or Closed (poller gone): end the
                // stream and let the client reconnect.
                Err(_) => None,
            }
        },
    )
    .take_until(shutdown.cancelled_owned());

    Sse::new(stream).keep_alive(
        // Comment ping through proxies that would otherwise reap an
        // idle connection — the feed can legitimately stay quiet for
        // long stretches.
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

fn make_event(enriched: EnrichedSignal) -> Result<Event, axum::Error> {
    let id = enriched.record.id.to_string();
    Event::default()
        .json_data(SignalResponse::from(enriched))
        .map(|event| event.id(id))
}
