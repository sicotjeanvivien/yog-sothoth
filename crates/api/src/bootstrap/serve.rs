//! The process's two tasks, and how they stop.

use axum::Router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_bootstrap::{Stop, handle_task_result};

use crate::application::SignalStreamPoller;
use crate::http::{CappedListener, IDLE_TIMEOUT, MAX_CONNECTIONS};

const HTTP_SERVER: &str = "http server";
const SIGNAL_POLLER: &str = "signal stream poller";

/// Serve `router` on `listener` and run the signal poller, until `shutdown` is
/// cancelled — then stop both, within [`yog_bootstrap::SHUTDOWN_GRACE`].
///
/// The same shape as the daemons' `run`: the first task to end, or the stop,
/// wins the `select!`; the other is told; [`Stop`] waits for both on one
/// deadline and names whichever outlives it.
///
/// ⚠️ **The server stops gracefully, and that alone would not stop it.**
/// `with_graceful_shutdown` stops accepting, then waits for every open
/// connection to finish — and an SSE connection never does on its own. It ends
/// because its stream reads the same token (`signal_sse`). The grace is what
/// still bounds the stop if some other response ever hangs.
pub(crate) async fn serve(
    listener: TcpListener,
    router: Router,
    poller: SignalStreamPoller,
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
    let mut poller_task = tokio::spawn(poller.run(shutdown.clone()));

    // The cancellation arm carries no verdict; a task that failed can still be
    // stopping when it fires, and `Stop` collects its error afterwards.
    let (ended, first) = tokio::select! {
        result = &mut server_task => (Some(HTTP_SERVER), handle_task_result(result, HTTP_SERVER)),
        result = &mut poller_task => (Some(SIGNAL_POLLER), handle_task_result(result, SIGNAL_POLLER)),
        () = shutdown.cancelled() => {
            info!("cancellation received — stopping");
            (None, Ok(()))
        }
    };

    // Whichever arm fired, the other task has to be told. Idempotent.
    shutdown.cancel();

    // The server first: it holds responses in flight, which are lost if cut.
    // The poller only skips a read of the feed it would redo on restart.
    let mut stop = Stop::new(first, ended);
    stop.settle(HTTP_SERVER, &mut server_task).await;
    stop.settle(SIGNAL_POLLER, &mut poller_task).await;
    stop.finish()
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
