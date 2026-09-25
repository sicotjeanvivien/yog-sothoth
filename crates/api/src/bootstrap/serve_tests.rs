//! A stop through the real `serve`, on a real socket, without a database.
//!
//! The client is a bare `TcpStream` writing HTTP/1.1 by hand: what is under
//! test is what the server does to an open connection, and a client library
//! would add its own idea of when a response is over.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::{Router, routing::get};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, broadcast};
use tokio_util::sync::CancellationToken;
use yog_bootstrap::SHUTDOWN_GRACE;

use super::serve;
use crate::application::{SignalService, SignalStreamPoller};
use crate::http::signal_sse;
use crate::testing::{MockMetadataRepo, MockPriceRepo, MockSignalRepo, PoolRepoOnce};

/// A poller that reads the feed at most once: its first tick fires at once,
/// usually before any client is connected, and the next one is an hour away.
fn idle_poller() -> SignalStreamPoller {
    let (sender, _) = broadcast::channel(8);
    SignalStreamPoller::new(
        Arc::new(MockSignalRepo::feed(Ok(None), Ok(vec![]))),
        sender,
        Duration::from_secs(3600),
    )
}

/// A signal service no test reaches: nothing is ever broadcast, so no event
/// is enriched.
fn unused_signal_service() -> Arc<SignalService> {
    Arc::new(SignalService::new(
        Arc::new(MockSignalRepo::empty()),
        Arc::new(PoolRepoOnce::with_pool(None)),
        Arc::new(MockMetadataRepo::empty()),
        Arc::new(MockPriceRepo::empty()),
    ))
}

/// Start `serve` on a free port; the address to connect to, and its handle.
async fn start(
    router: Router,
    shutdown: &CancellationToken,
) -> (
    std::net::SocketAddr,
    tokio::task::JoinHandle<anyhow::Result<()>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(serve(listener, router, idle_poller(), shutdown.clone()));
    (addr, handle)
}

async fn get_request(addr: std::net::SocketAddr, path: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    stream
}

/// Read until the end of the response headers.
async fn read_head(stream: &mut TcpStream) -> String {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        let n = stream.read(&mut byte).await.unwrap();
        assert_ne!(n, 0, "connection closed before the headers ended");
        head.push(byte[0]);
    }
    String::from_utf8(head).unwrap()
}

/// Mutation: drop the `take_until` in `signal_sse`, and the open stream holds
/// the graceful shutdown until the grace expires — `serve` then answers at
/// `SHUTDOWN_GRACE`, past the bound asserted here.
#[tokio::test]
async fn an_open_signal_stream_does_not_hold_the_stop() {
    let shutdown = CancellationToken::new();
    let (feed, _) = broadcast::channel(8);
    let (service, token) = (unused_signal_service(), shutdown.clone());
    let router = Router::new().route(
        "/api/signals/stream",
        get(move || {
            let (receiver, service, token) = (feed.subscribe(), service.clone(), token.clone());
            async move { signal_sse(receiver, service, token) }
        }),
    );
    let (addr, server) = start(router, &shutdown).await;

    let mut client = get_request(addr, "/api/signals/stream").await;
    let head = read_head(&mut client).await;
    assert!(
        head.starts_with("HTTP/1.1 200"),
        "stream not opened: {head}"
    );
    assert!(
        head.contains("text/event-stream"),
        "not an SSE response: {head}"
    );

    let asked = Instant::now();
    shutdown.cancel();
    let result = tokio::time::timeout(2 * SHUTDOWN_GRACE, server)
        .await
        .expect("serve never returned")
        .unwrap();
    let took = asked.elapsed();

    result.expect("a requested stop is not a failure");
    assert!(
        took < SHUTDOWN_GRACE / 2,
        "the open stream held the stop for {took:?}"
    );

    // And the client sees its response end, not hang: the stream closed.
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(1), client.read_to_end(&mut rest))
        .await
        .expect("the SSE response never ended")
        .unwrap();
}

/// `serve` returns only once the response in flight is written.
///
/// That order is the whole property: once `serve` returns, `main` returns and
/// the runtime is dropped, taking every connection task with it. A response
/// still being written at that point is cut, whatever axum was doing.
///
/// Mutation: stop the server by dropping it on cancellation instead of
/// `with_graceful_shutdown` — `serve` then returns while the handler is still
/// asleep, and the flag below is still down.
#[tokio::test]
async fn a_request_in_flight_is_answered_before_serve_returns() {
    let shutdown = CancellationToken::new();
    let started = Arc::new(Notify::new());
    let answered = Arc::new(AtomicBool::new(false));
    let (entered, done) = (started.clone(), answered.clone());
    let router = Router::new().route(
        "/slow",
        get(move || {
            let (entered, done) = (entered.clone(), done.clone());
            async move {
                entered.notify_one();
                tokio::time::sleep(Duration::from_millis(300)).await;
                done.store(true, Ordering::SeqCst);
                "done"
            }
        }),
    );
    let (addr, server) = start(router, &shutdown).await;

    let mut client = get_request(addr, "/slow").await;
    started.notified().await;
    shutdown.cancel();

    tokio::time::timeout(SHUTDOWN_GRACE, server)
        .await
        .expect("serve never returned")
        .unwrap()
        .expect("a requested stop is not a failure");
    assert!(
        answered.load(Ordering::SeqCst),
        "serve returned while a response was still being written"
    );

    let mut response = String::new();
    client.read_to_string(&mut response).await.unwrap();
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "not answered: {response:?}"
    );
    assert!(response.ends_with("done"), "body cut: {response:?}");
}
