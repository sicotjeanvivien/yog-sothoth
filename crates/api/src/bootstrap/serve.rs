//! The process's three tasks, and how they stop.

use axum::Router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_bootstrap::{Stop, handle_task_result};

use super::key_ring_reload::{KEY_RING_RELOAD, reload_key_ring};
use crate::application::{KeyRing, LivePollers};
use crate::http::{CappedListener, IDLE_TIMEOUT, MAX_CONNECTIONS};

const HTTP_SERVER: &str = "http server";
const LIVE_POLLERS: &str = "live pollers";
const KEY_RING: &str = "key ring";

/// Serves `router`, runs the live pollers and reloads the key ring until `shutdown`, then stops them within
/// [`yog_bootstrap::SHUTDOWN_GRACE`]. ⚠️ Graceful shutdown waits for every connection, and an SSE
/// one never ends alone: it ends because its stream reads the same token (`live_sse`).
pub(crate) async fn serve(
    listener: TcpListener,
    router: Router,
    pollers: LivePollers,
    key_ring: KeyRing,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let mut server_task = tokio::spawn(
        axum::serve(
            CappedListener::new(listener, MAX_CONNECTIONS, IDLE_TIMEOUT),
            router,
        )
        .with_graceful_shutdown(shutdown.clone().cancelled_owned())
        .into_future(),
    );
    let mut poller_task = tokio::spawn(pollers.run(shutdown.clone()));
    let mut key_ring_task =
        tokio::spawn(reload_key_ring(key_ring, KEY_RING_RELOAD, shutdown.clone()));

    // The cancellation arm carries no verdict; a task that failed can still be
    // stopping when it fires, and `Stop` collects its error afterwards.
    let (ended, first) = tokio::select! {
        result = &mut server_task => (Some(HTTP_SERVER), handle_task_result(result, HTTP_SERVER)),
        result = &mut poller_task => (Some(LIVE_POLLERS), handle_task_result(result, LIVE_POLLERS)),
        result = &mut key_ring_task => (Some(KEY_RING), handle_task_result(result, KEY_RING)),
        () = shutdown.cancelled() => {
            info!("cancellation received — stopping");
            (None, Ok(()))
        }
    };

    // Whichever arm fired, the other task has to be told. Idempotent.
    shutdown.cancel();

    // The server first: it holds responses in flight, which are lost if cut.
    // The pollers only skip a read of the tables they would redo on restart.
    let mut stop = Stop::new(first, ended);
    stop.settle(HTTP_SERVER, &mut server_task).await;
    stop.settle(LIVE_POLLERS, &mut poller_task).await;
    stop.settle(KEY_RING, &mut key_ring_task).await;
    stop.finish()
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
