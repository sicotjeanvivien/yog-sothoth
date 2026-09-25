use std::time::Duration;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    middleware::from_fn_with_state,
    routing::get,
};
use tower::ServiceExt;

use super::{HeavyRouteLimit, heavy_route_limit, request_deadline};

async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_millis(400)).await;
    "done"
}

async fn call(router: Router, path: &str) -> axum::response::Response {
    router
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

/// Two slots, three simultaneous slow requests: the third waits past its
/// wait and is refused — 503, `Retry-After`, Problem Details — while the two
/// holders complete.
#[tokio::test]
async fn a_slow_request_past_the_slots_is_refused_with_a_503() {
    let limit = HeavyRouteLimit::new(2, Duration::from_millis(100));
    let router = Router::new()
        .route("/slow", get(slow))
        .route_layer(from_fn_with_state(limit, heavy_route_limit));

    let (a, b, c) = tokio::join!(
        call(router.clone(), "/slow"),
        call(router.clone(), "/slow"),
        async {
            // Let the first two take the slots.
            tokio::time::sleep(Duration::from_millis(20)).await;
            call(router.clone(), "/slow").await
        },
    );

    assert_eq!(a.status(), StatusCode::OK);
    assert_eq!(b.status(), StatusCode::OK);
    assert_eq!(c.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(c.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(
        c.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
}

/// A slot is given back when its request ends: once the holders are done,
/// the next slow request runs.
#[tokio::test]
async fn a_freed_slot_serves_the_next_request() {
    let limit = HeavyRouteLimit::new(1, Duration::from_millis(100));
    let router = Router::new()
        .route("/slow", get(slow))
        .route_layer(from_fn_with_state(limit, heavy_route_limit));

    assert_eq!(call(router.clone(), "/slow").await.status(), StatusCode::OK);
    assert_eq!(call(router, "/slow").await.status(), StatusCode::OK);
}

/// A waiter that gets a slot within its wait is served, not refused: the wait
/// is what lets a page's own burst through.
#[tokio::test]
async fn a_request_that_gets_a_slot_within_its_wait_is_served() {
    let limit = HeavyRouteLimit::new(1, Duration::from_secs(2));
    let router = Router::new()
        .route("/slow", get(slow))
        .route_layer(from_fn_with_state(limit, heavy_route_limit));

    let (a, b) = tokio::join!(call(router.clone(), "/slow"), call(router, "/slow"));

    assert_eq!(a.status(), StatusCode::OK);
    assert_eq!(b.status(), StatusCode::OK);
}

/// A handler slower than the deadline answers 503 instead of holding the
/// connection.
#[tokio::test]
async fn a_request_past_its_deadline_is_a_503() {
    let router = Router::new()
        .route("/slow", get(slow))
        .layer(from_fn_with_state(
            Duration::from_millis(50),
            request_deadline,
        ));

    let response = call(router, "/slow").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(response.headers().contains_key(header::RETRY_AFTER));
}

/// The deadline bounds producing the response, not streaming its body — which
/// is what keeps it from cutting `/api/signals/stream`. A body that takes
/// four times the deadline arrives whole.
#[tokio::test]
async fn the_deadline_does_not_cut_a_streaming_body() {
    async fn streaming() -> Body {
        let chunks = futures_util::stream::unfold(0u8, |n| async move {
            if n == 4 {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            Some((Ok::<_, std::io::Error>(format!("{n}")), n + 1))
        });
        Body::from_stream(chunks)
    }
    let router = Router::new()
        .route("/stream", get(streaming))
        .layer(from_fn_with_state(
            Duration::from_millis(50),
            request_deadline,
        ));

    let response = call(router, "/stream").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"0123");
}

/// Past the cap, a connection is not accepted until one closes: the second
/// client's handshake completes in the kernel's queue, but the process does
/// not take it — and takes it as soon as the first one leaves.
///
/// Mutation: accept without taking a slot, and the second accept returns at
/// once.
#[tokio::test]
async fn past_the_connection_cap_a_client_waits_until_one_closes() {
    use axum::serve::Listener;
    use tokio::net::{TcpListener, TcpStream};

    use super::CappedListener;

    let inner = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = inner.local_addr().unwrap();
    let mut listener = CappedListener::new(inner, 1);

    let _first_client = TcpStream::connect(addr).await.unwrap();
    let (first, _) = listener.accept().await;
    let _second_client = TcpStream::connect(addr).await.unwrap();

    assert!(
        tokio::time::timeout(Duration::from_millis(200), listener.accept())
            .await
            .is_err(),
        "the cap is 1 and the first connection is still open"
    );

    drop(first);
    tokio::time::timeout(Duration::from_secs(1), listener.accept())
        .await
        .expect("the freed slot takes the waiting client");
}
