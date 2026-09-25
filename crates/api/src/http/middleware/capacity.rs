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

use std::future::Future;
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
use tokio::time::{Instant, Sleep};

use crate::application::WorkSlots;
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

/// How much expensive database work may run at once: the slow routes and the
/// shared-result computations together ([`WorkSlots`]).
///
/// Below the pool's 10 connections ([`yog_persistence::Database::DEFAULT_MAX_CONNECTIONS`]):
/// with the signal poller's one, the light routes always keep 3.
pub(crate) const HEAVY_ROUTE_PERMITS: usize = 6;

/// How long expensive work waits for a slot before being refused.
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

/// How long a connection may go without a byte in either direction before it
/// is closed, giving its [`MAX_CONNECTIONS`] slot back.
///
/// Without it the cap would be a cheaper outage than the one it prevents:
/// axum's `serve` sets no header-read nor keep-alive timeout, so 400 silent
/// sockets would hold every slot for good. It counts **both** directions:
/// an SSE client sends nothing after its request, and the 15 s keep-alive
/// ping is what keeps its stream alive. Above that ping, and above the worst
/// request ([`HEAVY_ROUTE_WAIT`] + [`REQUEST_TIMEOUT`]), during which nothing
/// is written.
///
/// ⚠️ **It bounds the outage, it does not remove it.** Measured on
/// 25 September 2026: 400 silent sockets, and `/healthz` answered after
/// 28 s, the next request in 16 ms. Someone who can reach this process
/// directly can repeat that every 30 s. The design assumes they cannot: in
/// production only Caddy connects here, and it opens an upstream connection
/// once it holds a complete request — silent sockets stop at the edge.
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

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

/// Run a slow route only once it holds one of the [`WorkSlots`]; refuse it
/// if none frees up within the wait.
///
/// One semaphore for every slow route and for the cached computations
/// (`application/work_slots.rs`): `tower::limit::ConcurrencyLimitLayer` would
/// give each route its own semaphore under axum, and refuse outside Problem
/// Details.
pub(crate) async fn heavy_route_limit(
    State(slots): State<WorkSlots>,
    request: Request,
    next: Next,
) -> Response {
    match slots.acquire().await {
        Some(_slot) => next.run(request).await,
        None => ApiError::Unavailable("the API is at capacity; retry shortly").into_response(),
    }
}

/// A `TcpListener` that holds at most `max` connections at once.
///
/// It takes a slot **before** accepting: past the cap, a connection waits in
/// the kernel's queue rather than in the process. The slot travels with the
/// accepted stream and is released when the connection closes.
///
/// A connection idle for `idle` — no byte read, none written — is closed
/// ([`IDLE_TIMEOUT`]), so a silent socket cannot keep its slot.
pub(crate) struct CappedListener {
    inner: TcpListener,
    slots: Arc<Semaphore>,
    idle: Duration,
}

impl CappedListener {
    pub(crate) fn new(inner: TcpListener, max: usize, idle: Duration) -> Self {
        Self {
            inner,
            slots: Arc::new(Semaphore::new(max)),
            idle,
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
                idle: self.idle,
                deadline: Box::pin(tokio::time::sleep(self.idle)),
            },
            addr,
        )
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

/// An accepted connection, the slot it holds until it closes, and the
/// deadline its next byte must beat.
pub(crate) struct CappedStream {
    stream: TcpStream,
    _slot: OwnedSemaphorePermit,
    idle: Duration,
    deadline: Pin<Box<Sleep>>,
}

impl CappedStream {
    /// A byte went through, one way or the other: the connection is live.
    fn touch(&mut self) {
        let next = Instant::now() + self.idle;
        self.deadline.as_mut().reset(next);
    }

    /// Account for a write's outcome: bytes out make the connection live; a
    /// write still waiting past the deadline ends it.
    ///
    /// The write side needs its own check. A client that stops reading
    /// leaves every write pending, and while hyper sends a response it may
    /// never read again — the read-side check alone let such a connection
    /// keep its slot forever (found in review).
    fn written(
        &mut self,
        cx: &mut Context<'_>,
        outcome: Poll<io::Result<usize>>,
    ) -> Poll<io::Result<usize>> {
        match outcome {
            Poll::Ready(Ok(n)) if n > 0 => {
                self.touch();
                outcome
            }
            Poll::Pending if self.idle_expired(cx) => Poll::Ready(Err(idle_error())),
            _ => outcome,
        }
    }

    /// Whether the deadline has passed. Registers the waker when it has not,
    /// so a stalled connection is woken — and closed — when it does.
    fn idle_expired(&mut self, cx: &mut Context<'_>) -> bool {
        self.deadline.as_mut().poll(cx).is_ready()
    }
}

impl AsyncRead for CappedStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        match Pin::new(&mut this.stream).poll_read(cx, buf) {
            Poll::Ready(outcome) => {
                if buf.filled().len() > before {
                    this.touch();
                }
                Poll::Ready(outcome)
            }
            // Nothing to read yet: close the connection if it has been idle
            // too long. The sleep registers the waker, so an idle connection
            // is woken — and closed — when the deadline passes.
            Poll::Pending if this.idle_expired(cx) => Poll::Ready(Err(idle_error())),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for CappedStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let outcome = Pin::new(&mut this.stream).poll_write(cx, buf);
        this.written(cx, outcome)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let outcome = Pin::new(&mut this.stream).poll_write_vectored(cx, bufs);
        this.written(cx, outcome)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.stream).poll_flush(cx) {
            Poll::Pending if this.idle_expired(cx) => Poll::Ready(Err(idle_error())),
            outcome => outcome,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/// The error an idle connection ends with.
fn idle_error() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "connection idle past IDLE_TIMEOUT")
}

#[cfg(test)]
#[path = "capacity_tests.rs"]
mod tests;
