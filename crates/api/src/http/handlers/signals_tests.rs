use std::sync::Arc;

use axum::http::{StatusCode, header};
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;

use super::open_signal_stream;
use crate::application::EnrichedSignal;
use crate::testing::get;

/// Mutation: the handler extracts a bare `Query`, whose refusal is plain text.
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
