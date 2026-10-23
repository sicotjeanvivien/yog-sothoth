//! Capacity bounds: what keeps one client from taking the API down for everyone. Two are
//! applied outside the router: the SSE cap by the stream handler, which holds its permit for
//! the life of the stream, and the connection cap by [`CappedListener`].

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

/// How long a request may take to produce its response — not its body, so the SSE stream
/// is not cut. ⚠️ Above [`STATEMENT_TIMEOUT`]: a statement Postgres cancels surfaces first.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How long Postgres lets one of the API's statements run: twice the slowest route at rest.
/// Only the server can enforce it: a request cut client-side leaves its statement running.
pub(crate) const STATEMENT_TIMEOUT: Duration = Duration::from_secs(8);

/// How much expensive database work may run at once ([`WorkSlots`]). ⚠️ Below the pool's
/// [`yog_persistence::Database::DEFAULT_MAX_CONNECTIONS`]: with the signal poller's, the light routes keep 3.
pub(crate) const WORK_SLOTS: usize = 6;

/// How long expensive work waits for a slot before being refused: long enough for one
/// page's burst, short enough that a refused client learns it quickly.
pub(crate) const WORK_SLOT_WAIT: Duration = Duration::from_secs(2);

/// How many `/api/signals/stream` connections may be open at once: half of
/// [`MAX_CONNECTIONS`], so streams never take the connections ordinary requests need.
pub(crate) const SSE_MAX_STREAMS: usize = 200;

/// How many connections the process holds at once; the next wait in the kernel's accept queue.
/// ⚠️ Capping streams is not enough: refused connections cost memory the allocator keeps.
/// The bound holds across bursts only with `MALLOC_ARENA_MAX=2`, set in the `yog-api` image.
pub(crate) const MAX_CONNECTIONS: usize = 400;

/// How long a connection may go without a byte either way before it is closed: above the SSE
/// keep-alive ping and the worst request. ⚠️ It bounds a silent-socket outage, not removes it:
/// the design assumes only Caddy, which forwards complete requests, reaches this process.
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

/// Runs a slow route only once it holds one of the [`WorkSlots`], refused otherwise. Not
/// `ConcurrencyLimitLayer`: one semaphore per route under axum, refusing outside Problem Details.
pub(crate) async fn work_slot_limit(
    State(slots): State<WorkSlots>,
    request: Request,
    next: Next,
) -> Response {
    match slots.acquire().await {
        Some(_slot) => next.run(request).await,
        None => ApiError::Unavailable("the API is at capacity; retry shortly").into_response(),
    }
}

/// A `TcpListener` holding at most `max` connections. ⚠️ The slot is taken before accepting,
/// so past the cap a connection waits in the kernel, not the process. A connection idle for
/// `idle` either way is closed ([`IDLE_TIMEOUT`]).
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

    /// Bytes out make the connection live; a write still pending past the deadline ends it.
    /// ⚠️ A client that stops reading leaves writes pending while hyper never reads again.
    fn on_written(
        &mut self,
        cx: &mut Context<'_>,
        outcome: Poll<io::Result<usize>>,
    ) -> Poll<io::Result<usize>> {
        match outcome {
            Poll::Ready(Ok(n)) if n > 0 => {
                self.touch();
                outcome
            }
            Poll::Pending if self.poll_idle_expired(cx) => Poll::Ready(Err(idle_error())),
            _ => outcome,
        }
    }

    /// Whether the deadline has passed. Registers the waker when it has not,
    /// so a stalled connection is woken — and closed — when it does.
    fn poll_idle_expired(&mut self, cx: &mut Context<'_>) -> bool {
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
            Poll::Pending if this.poll_idle_expired(cx) => Poll::Ready(Err(idle_error())),
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
        this.on_written(cx, outcome)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let outcome = Pin::new(&mut this.stream).poll_write_vectored(cx, bufs);
        this.on_written(cx, outcome)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.stream).poll_flush(cx) {
            Poll::Pending if this.poll_idle_expired(cx) => Poll::Ready(Err(idle_error())),
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
