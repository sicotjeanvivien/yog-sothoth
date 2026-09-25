//! Capacity bounds: what keeps one client from taking the API down for
//! everyone.
//!
//! Measured on 25 September 2026, before these bounds existed, with the dev
//! database behind a 10-connection pool:
//!
//! - 30 parallel `/api/pools/top` from one client → 10 answered `500`, and
//!   `/api/tokens/{mint}` (4 ms at rest) answered `500` after 5.0 s — the
//!   slow routes (1.8–3.7 s each) held every connection;
//! - ~500 `/api/signals/stream` connections → the process OOM-killed under
//!   the production `mem_limit` of 32 MiB.
//!
//! Every number below is named once, here, with the measurement it answers.
//! Two are applied outside the router: the SSE cap by the stream handler,
//! which holds its permit for the life of the stream, and the connection cap
//! by [`CappedListener`], which holds one for the life of the connection.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
    serve::Listener,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::http::error::ApiError;

/// How long a request may take to produce its response.
///
/// Above [`STATEMENT_TIMEOUT`], so that a statement Postgres cancels surfaces
/// as the repository error it is, before this deadline cuts the request. It
/// bounds the response's *production*, not its body: the SSE stream, which
/// answers its headers at once, is not cut by it.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How long Postgres lets one of the API's statements run.
///
/// The slowest route measured at rest took 3.7 s; twice that leaves room for
/// a loaded database before a statement is abandoned. Only the server can
/// enforce it: a request cut client-side leaves its statement running.
pub(crate) const STATEMENT_TIMEOUT: Duration = Duration::from_secs(8);

/// How many slow requests may run at once.
///
/// Below the pool's 10 connections ([`yog_persistence::Database::DEFAULT_MAX_CONNECTIONS`]),
/// so the light routes and the signal poller always keep 4.
pub(crate) const HEAVY_ROUTE_PERMITS: usize = 6;

/// How long a slow request waits for a slot before being refused.
///
/// Long enough for the bursts a page makes on its own (a dashboard loads
/// three slow routes at once), short enough that a refused client learns so
/// quickly rather than piling up.
pub(crate) const HEAVY_ROUTE_WAIT: Duration = Duration::from_secs(2);

/// How many `/api/signals/stream` connections may be open at once.
///
/// An open stream costs ~62 KiB (measured 25 September 2026: 2.4 MiB idle,
/// 14.75 MiB with 200 streams). Half of [`MAX_CONNECTIONS`], so that open
/// streams never take the connections ordinary requests need.
pub(crate) const SSE_MAX_STREAMS: usize = 200;

/// How many connections the process holds at once; the next ones wait in the
/// kernel's accept queue, where they cost the process no memory.
///
/// Capping streams was not enough. A burst of connections costs memory even
/// when every one of them is refused, and the allocator keeps that peak:
/// measured on 25 September 2026, 200 then 400 refused connections took the
/// process from 14.75 to 29.9 MiB and it stayed there, and 1000 at once got
/// it OOM-killed under a 32 MiB limit with the stream cap in place. Only a
/// bound on connections bounds the peak.
///
/// The bound alone did not hold across bursts: glibc kept each peak in a new
/// per-thread arena, and three rounds of 1000 opens went 29.7 → 49 MiB →
/// OOM under 64 MiB. With `MALLOC_ARENA_MAX=2`, set in the `yog-api` image
/// (`docker/backend.Dockerfile`), twelve rounds climb in steps to 44.1 MiB
/// and hold there over the last three — 20 MiB inside the 64 MiB
/// `mem_limit` of `docker-compose.prod.yml`.
pub(crate) const MAX_CONNECTIONS: usize = 400;

/// Refuse a request that has not produced its response within `limit`.
pub(crate) async fn request_deadline(
    State(limit): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(limit, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::Unavailable("the request took too long; retry shortly").into_response(),
    }
}

/// The slots shared by every slow route, process-wide.
///
/// One semaphore behind an `Arc`, handed to the middleware of each slow
/// route: `tower::limit::ConcurrencyLimitLayer` would give each route its own
/// semaphore under axum, and refuse outside Problem Details.
#[derive(Clone)]
pub(crate) struct HeavyRouteLimit {
    slots: Arc<Semaphore>,
    wait: Duration,
}

impl HeavyRouteLimit {
    pub(crate) fn new(permits: usize, wait: Duration) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(permits)),
            wait,
        }
    }
}

/// Run a slow route only once it holds a slot; refuse it if none frees up
/// within the wait.
pub(crate) async fn heavy_route_limit(
    State(limit): State<HeavyRouteLimit>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(limit.wait, limit.slots.acquire_owned()).await {
        Ok(Ok(_slot)) => next.run(request).await,
        // Timed out, or the semaphore was closed (it never is).
        _ => ApiError::Unavailable("the API is at capacity; retry shortly").into_response(),
    }
}

/// A `TcpListener` that holds at most `max` connections at once.
///
/// It takes a slot **before** accepting: past the cap, a connection waits in
/// the kernel's queue rather than in the process. The slot travels with the
/// accepted stream and is released when the connection closes.
pub(crate) struct CappedListener {
    inner: TcpListener,
    slots: Arc<Semaphore>,
}

impl CappedListener {
    pub(crate) fn new(inner: TcpListener, max: usize) -> Self {
        Self {
            inner,
            slots: Arc::new(Semaphore::new(max)),
        }
    }
}

impl Listener for CappedListener {
    type Io = CappedStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let slot = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .expect("the connection semaphore is never closed");
        // Axum's own accept: it logs and retries accept errors.
        let (stream, addr) = Listener::accept(&mut self.inner).await;
        (
            CappedStream {
                stream,
                _slot: slot,
            },
            addr,
        )
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

/// An accepted connection and the slot it holds until it closes.
pub(crate) struct CappedStream {
    stream: TcpStream,
    _slot: OwnedSemaphorePermit,
}

impl AsyncRead for CappedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for CappedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[path = "capacity_tests.rs"]
mod tests;
