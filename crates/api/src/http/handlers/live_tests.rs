use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use yog_core::domain::Protocol;

use super::{SignalResponse, TopicStream, live_sse, open_live, signals_topic};
use crate::application::{EnrichedSignal, EnrichedToken};
use crate::http::{build_router, cursor::LiveCursor, dto::request::LiveTopic};
use crate::testing::{app_state, get, make_metadata, make_signal_record, pk};

/// Mutation: a topic the stream does not serve accepted — the connection opens and stays silent.
#[tokio::test]
async fn a_connection_it_cannot_serve_is_refused_in_problem_details() {
    for (uri, reason) in [
        ("/api/live", "missing field `topics`"),
        ("/api/live?topics=", "`topics` must not be empty"),
        (
            "/api/live?topics=swaps",
            "`swaps` is not streamed; served: signals",
        ),
        (
            "/api/live?topics=liquidity-changes",
            "`liquidity-changes` is not streamed",
        ),
        ("/api/live?topics=signals,signals", "names `signals` twice"),
        ("/api/live?topics=signals,", "`` is not streamed"),
        ("/api/live?topics=signals&pool=x", "unknown field `pool`"),
        (
            "/api/live/protocols/raydium?topics=signals",
            "unknown protocol: raydium",
        ),
    ] {
        let (status, content_type, detail) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {detail}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
        assert!(detail.contains(reason), "{uri}: {detail}");
    }
}

/// The control of the refusals above, with a `Last-Event-ID` a resume would send: it is not
/// read, so even one that decodes to nothing opens the stream.
#[tokio::test]
async fn a_served_connection_opens_whatever_its_last_event_id() {
    for uri in [
        "/api/live?topics=signals",
        "/api/live/protocols/meteora_damm_v2?topics=signals",
    ] {
        let request = Request::builder()
            .uri(uri)
            .header("last-event-id", "not-a-cursor")
            .body(Body::empty())
            .unwrap();
        let response = build_router(app_state(), vec![])
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream",
            "{uri}"
        );
    }
}

/// The whole body of a connection on `topics`, read until it ends, and whether its slot came back.
async fn read_live(topics: Vec<TopicStream>) -> (String, bool) {
    let slots = Arc::new(Semaphore::new(1));
    let slot = slots.clone().try_acquire_owned().expect("a free slot");
    let response = axum::response::IntoResponse::into_response(live_sse(
        topics,
        slot,
        CancellationToken::new(),
    ));
    assert_eq!(
        slots.available_permits(),
        0,
        "the connection holds its slot"
    );

    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("the connection ends")
    .expect("the body reads");
    (
        String::from_utf8(body.to_vec()).expect("text"),
        slots.available_permits() == 1,
    )
}

fn enriched(id: i64, seed: u8, protocol: Protocol) -> EnrichedSignal {
    let token = |mint, symbol| EnrichedToken {
        mint: Some(mint),
        metadata: Some(make_metadata(mint, symbol)),
    };
    let mut record = make_signal_record(id, pk(seed));
    record.signal.protocol = protocol;
    EnrichedSignal {
        record,
        token_a: token(pk(seed + 10), "AAA"),
        token_b: token(pk(seed + 20), "BBB"),
    }
}

/// The signals broadcast, then the source closed: what a connection would have received.
fn broadcast_then_close(signals: &[EnrichedSignal]) -> broadcast::Receiver<Arc<EnrichedSignal>> {
    let (feed, receiver) = broadcast::channel(8);
    for signal in signals {
        feed.send(Arc::new(signal.clone())).expect("a subscriber");
    }
    receiver
}

fn events(body: &str) -> Vec<&str> {
    body.split("\n\n")
        .filter(|event| !event.is_empty())
        .collect()
}

fn field<'a>(event: &'a str, name: &str) -> &'a str {
    event
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("no `{name}` in {event:?}"))
}

/// Mutation: the `id` reduced to the signal's own, the event left unnamed, `data` not the
/// signal's response, or the signal sent bare.
#[tokio::test]
async fn each_signal_comes_named_by_its_topic_with_the_connections_cursor() {
    let signals = [
        enriched(41, 1, Protocol::MeteoraDammV2),
        enriched(42, 2, Protocol::MeteoraDammV2),
    ];
    let receiver = broadcast_then_close(&signals);

    let (body, slot_back) = read_live(vec![signals_topic(receiver, None)]).await;

    assert!(slot_back);
    let events = events(&body);
    assert_eq!(events.len(), 2, "{body}");
    for (event, signal) in events.iter().zip(&signals) {
        assert_eq!(field(event, "event"), "signals");
        let cursor = LiveCursor::decode(field(event, "id")).expect("an opaque cursor");
        assert_eq!(cursor.position(LiveTopic::Signals), Some(signal.record.id));
        let data: serde_json::Value = serde_json::from_str(field(event, "data")).expect("JSON");
        let expected = serde_json::to_value(SignalResponse::from(signal.clone())).expect("JSON");
        assert_eq!(data, expected);
        assert_eq!(data["tokenA"]["symbol"], "AAA");
        assert_eq!(data["tokenB"]["symbol"], "BBB");
    }
}

/// Mutation: the protocol filter dropped — DLMM's signal reaches a DAMM v2 connection.
#[tokio::test]
async fn a_protocols_connection_carries_its_signals_only() {
    let signals = [
        enriched(41, 1, Protocol::MeteoraDlmm),
        enriched(42, 2, Protocol::MeteoraDammV2),
    ];

    let (body, _) = read_live(vec![signals_topic(
        broadcast_then_close(&signals),
        Some(Protocol::MeteoraDammV2),
    )])
    .await;
    let events = events(&body);
    assert_eq!(events.len(), 1, "{body}");
    let data: serde_json::Value = serde_json::from_str(field(events[0], "data")).expect("JSON");
    assert_eq!(data["id"], 42);

    // The control: without a protocol, both come.
    let (body, _) = read_live(vec![signals_topic(broadcast_then_close(&signals), None)]).await;
    assert_eq!(self::events(&body).len(), 2, "{body}");
}

/// Mutation: opening without taking a slot — of two slots, the third connection opens. Closing
/// one lets the next client in. And the topics built before the slot is taken — the refused
/// client subscribes too.
#[tokio::test]
async fn connections_past_the_cap_are_refused_until_one_closes() {
    let slots = Arc::new(Semaphore::new(2));
    let (feed, _) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    let shutdown = CancellationToken::new();
    let subscriptions = std::cell::Cell::new(0);
    let open = || {
        open_live(
            slots.clone(),
            || {
                subscriptions.set(subscriptions.get() + 1);
                vec![signals_topic(feed.subscribe(), None)]
            },
            shutdown.clone(),
        )
    };

    let first = open();
    let second = open();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);

    let refused = open();
    assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(refused.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(
        refused.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(
        subscriptions.get(),
        2,
        "the refused client never subscribes"
    );

    // The client leaves: its response, and the connection holding the slot, drop.
    drop(first);
    assert_eq!(open().status(), StatusCode::OK);
}

/// Mutation: a client past the topic's buffer kept on the connection. The source stays open here.
#[tokio::test]
async fn a_client_too_late_ends_its_connection_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel(1);
    for id in 1..=3 {
        feed.send(Arc::new(EnrichedSignal::bare(make_signal_record(
            id,
            pk(1),
        ))))
        .expect("a subscriber");
    }

    let (body, slot_back) = read_live(vec![signals_topic(receiver, None)]).await;

    assert_eq!(body, "");
    assert!(slot_back);
}

/// Mutation: a closed source no longer ending the connection, or the slot never given back.
#[tokio::test]
async fn a_closed_source_ends_the_connection_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    drop(feed);

    let (body, slot_back) = read_live(vec![signals_topic(receiver, None)]).await;

    assert_eq!(body, "");
    assert!(slot_back);
}
