use std::sync::Arc;
use std::time::Duration;

use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use solana_pubkey::Pubkey;
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;

use super::{SignalResponse, open_signal_stream, signal_sse};
use crate::application::EnrichedSignal;
use crate::testing::{get, make_signal_record};

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
    let pool = crate::testing::pk(1);
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

/// The whole body of a stream, read until it ends, with whether its slot came back.
async fn read_stream(receiver: broadcast::Receiver<Arc<EnrichedSignal>>) -> (String, usize) {
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
        slots.available_permits(),
    )
}

fn field<'a>(event: &'a str, name: &str) -> &'a str {
    event
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("no `{name}` in {event:?}"))
}

/// Mutation: `event.id` removed or holding another value, or `data` not the signal's response.
#[tokio::test]
async fn a_client_receives_each_signal_with_its_id_and_its_response() {
    let (feed, receiver) = broadcast::channel(8);
    let records = [
        make_signal_record(41, Pubkey::new_unique()),
        make_signal_record(42, Pubkey::new_unique()),
    ];
    for record in &records {
        feed.send(Arc::new(EnrichedSignal::bare(record.clone())))
            .expect("a subscriber");
    }
    drop(feed);

    let (body, _) = read_stream(receiver).await;

    let events: Vec<&str> = body
        .split("\n\n")
        .filter(|event| !event.is_empty())
        .collect();
    assert_eq!(events.len(), 2, "{body}");
    for (event, record) in events.iter().zip(&records) {
        assert_eq!(field(event, "id"), record.id.to_string());
        let data: serde_json::Value = serde_json::from_str(field(event, "data")).expect("JSON");
        let expected =
            serde_json::to_value(SignalResponse::from(EnrichedSignal::bare(record.clone())))
                .expect("JSON");
        assert_eq!(data, expected);
    }
}

/// Mutation: a client past the channel's capacity kept on the stream: it would miss signals without
/// knowing, where a reconnection reloads the feed. The source stays open here.
#[tokio::test]
async fn a_client_too_late_ends_its_stream_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel(1);
    for id in 1..=3 {
        feed.send(Arc::new(EnrichedSignal::bare(make_signal_record(
            id,
            Pubkey::new_unique(),
        ))))
        .expect("a subscriber");
    }

    let (body, slots) = read_stream(receiver).await;

    assert_eq!(body, "");
    assert_eq!(slots, 1);
    drop(feed);
}

/// Mutation: a closed source no longer ending the stream, or the slot kept once it ends: past the
/// cap, no client could then connect again.
#[tokio::test]
async fn a_closed_source_ends_the_stream_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    drop(feed);

    let (body, slots) = read_stream(receiver).await;

    assert_eq!(body, "");
    assert_eq!(slots, 1);
}
