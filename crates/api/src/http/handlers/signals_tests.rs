use std::sync::Arc;

use axum::http::{StatusCode, header};
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;

use super::open_signal_stream;
use crate::application::EnrichedSignal;

/// Two slots: two streams open, the third is refused — `503`, `Retry-After`,
/// Problem Details — and closing one stream lets the next client in.
///
/// Mutation: open without taking a slot, and the third stream opens.
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
