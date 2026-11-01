use std::sync::Arc;
use std::time::Duration;

use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;

use super::{SignalResponse, open_signal_stream, signal_sse};
use crate::application::{EnrichedSignal, EnrichedToken};
use crate::testing::{get, make_metadata, make_signal_record, pk};

/// Mutation: the router without its Problem Details layer, axum's refusal left in plain text.
#[tokio::test]
async fn a_query_the_api_cannot_parse_is_refused_in_problem_details() {
    for (uri, field, reason) in [
        (
            "/api/signals?severity=panic",
            "severity",
            "unknown variant `panic`",
        ),
        ("/api/signals?limit=abc", "limit", "invalid digit"),
        ("/api/signals?severity=", "severity", "unknown variant ``"),
    ] {
        let (status, content_type, detail) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
        assert!(
            detail.contains(&format!("{field}: {reason}")),
            "{uri}: {detail}"
        );
    }
}

/// The control of the refusals above: the same route and state serve a valid query.
#[tokio::test]
async fn a_valid_query_is_served() {
    let (status, content_type, _) = get("/api/signals?severity=info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "application/json");
}

/// Mutation: opening without taking a slot — of two slots, the third stream opens. Closing
/// one lets the next client in.
#[tokio::test]
async fn streams_past_the_cap_are_refused_until_one_closes() {
    let slots = Arc::new(Semaphore::new(2));
    let (feed, _) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    let shutdown = CancellationToken::new();
    let open = || open_signal_stream(slots.clone(), feed.subscribe(), shutdown.clone());

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

    // The client leaves: its response, and the stream holding the slot, drop.
    drop(first);
    assert_eq!(open().status(), StatusCode::OK);
}

/// Mutation: the unknown keys let through — `?pool=` answered with every protocol's signals.
#[tokio::test]
async fn a_key_the_signals_do_not_take_is_refused() {
    let pool = pk(1);
    for uri in [
        format!("/api/signals?pool={pool}"),
        format!("/api/protocols/meteora_damm_v2/signals?pool={pool}"),
    ] {
        let (status, content_type, body) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
    let (status, _, body) = get("/api/signals?severity=critical&limit=5").await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// The whole body of a stream, read until it ends, and whether its slot came back.
async fn read_stream(receiver: broadcast::Receiver<Arc<EnrichedSignal>>) -> (String, bool) {
    let slots = Arc::new(Semaphore::new(1));
    let slot = slots.clone().try_acquire_owned().expect("a free slot");
    let response = signal_sse(receiver, slot, CancellationToken::new()).into_response();
    assert_eq!(slots.available_permits(), 0, "the stream holds its slot");

    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("the stream ends")
    .expect("the body reads");
    (
        String::from_utf8(body.to_vec()).expect("text"),
        slots.available_permits() == 1,
    )
}

fn enriched(id: i64, seed: u8) -> EnrichedSignal {
    let token = |mint, symbol| EnrichedToken {
        mint: Some(mint),
        metadata: Some(make_metadata(mint, symbol)),
    };
    EnrichedSignal {
        record: make_signal_record(id, pk(seed)),
        token_a: token(pk(seed + 10), "AAA"),
        token_b: token(pk(seed + 20), "BBB"),
    }
}

fn field<'a>(event: &'a str, name: &str) -> &'a str {
    event
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("no `{name}` in {event:?}"))
}

/// Mutation: `event.id` removed or holding another value, `data` not the signal's response, or the
/// signal sent bare or its tokens swapped; or the event named, which `onmessage` never receives.
#[tokio::test]
async fn a_client_receives_each_signal_with_its_id_and_its_response() {
    let (feed, receiver) = broadcast::channel(8);
    let signals = [enriched(41, 1), enriched(42, 2)];
    for signal in &signals {
        feed.send(Arc::new(signal.clone())).expect("a subscriber");
    }
    drop(feed);

    let (body, slot_back) = read_stream(receiver).await;

    assert!(slot_back);

    let events: Vec<&str> = body
        .split("\n\n")
        .filter(|event| !event.is_empty())
        .collect();
    assert_eq!(events.len(), 2, "{body}");
    for (event, signal) in events.iter().zip(&signals) {
        assert_eq!(field(event, "id"), signal.record.id.to_string());
        assert!(
            !event.lines().any(|line| line.starts_with("event:")),
            "a named event never reaches `onmessage`: {event}"
        );
        let data: serde_json::Value = serde_json::from_str(field(event, "data")).expect("JSON");
        let expected = serde_json::to_value(SignalResponse::from(signal.clone())).expect("JSON");
        assert_eq!(data, expected);
        assert_eq!(data["id"], signal.record.id);
        assert_eq!(data["tokenA"]["symbol"], "AAA");
        assert_eq!(data["tokenB"]["symbol"], "BBB");
    }
}

/// Mutation: a client past the channel's capacity kept on the stream. The source stays open here.
#[tokio::test]
async fn a_client_too_late_ends_its_stream_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel(1);
    for id in 1..=3 {
        feed.send(Arc::new(EnrichedSignal::bare(make_signal_record(
            id,
            pk(1),
        ))))
        .expect("a subscriber");
    }

    let (body, slot_back) = read_stream(receiver).await;

    assert_eq!(body, "");
    assert!(slot_back);
}

/// Mutation: a closed source no longer ending the stream, or the slot never given back.
#[tokio::test]
async fn a_closed_source_ends_the_stream_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    drop(feed);

    let (body, slot_back) = read_stream(receiver).await;

    assert_eq!(body, "");
    assert!(slot_back);
}
