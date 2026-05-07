use anyhow::Context;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::{Duration, timeout},
};
use tracing::{error, info};

use crate::bootstrap::{AppState, Config};
use crate::interface::{HttpError, HttpResponse, Router, decode_request};

/// Maximum number of concurrent in-flight HTTP connections.
///
/// Backpressure mechanism: when the limit is reached, new TCP
/// connections wait for a permit before being processed. Prevents
/// unbounded task spawning under load.
const MAX_CONCURRENT_CONNECTIONS: usize = 100;

/// Per-request timeout. Read + dispatch must complete within this
/// window or the connection is closed with a 408 Request Timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct Server {
    router: Arc<Router>,
    tcp_listener: TcpListener,
    limiter: Arc<Semaphore>,
}

impl Server {
    /// Build the server from the application app_state and config.
    ///
    /// Consumes both: the config is used here and not stored, the
    /// app_state is consumed by the router builder which captures
    /// each handler's dependencies in its closures.
    pub(crate) async fn init(app_state: AppState, config: Config) -> anyhow::Result<Self> {
        let app_state = Arc::new(app_state);
        let router = crate::bootstrap::build_router(app_state).await;
        let limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

        let tcp_listener = TcpListener::bind(config.bind_addr)
            .await
            .with_context(|| format!("failed to bind TCP listener on {}", config.bind_addr))?;
        info!(addr = %config.bind_addr, "API server listening");

        Ok(Server {
            router,
            tcp_listener,
            limiter,
        })
    }

    pub(crate) async fn run(self) {
        let server = async move {
            loop {
                match self.tcp_listener.accept().await {
                    Ok((stream, addr)) => {
                        self.handle_accept(stream, addr).await;
                    }
                    Err(e) => {
                        error!(error = %e, "accept error");
                    }
                }
            }
        };

        tokio::select! {
            _ = server => {}
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown signal received — stopping server");
            }
        }

        info!("server stopped");
    }

    async fn handle_accept(&self, stream: TcpStream, addr: SocketAddr) {
        info!(client = %addr, "new connection");
        let router = Arc::clone(&self.router);
        let permit = match self.limiter.clone().acquire_owned().await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "failed to acquire connection permit");
                return;
            }
        };

        tokio::spawn(async move {
            let _permit = permit;
            handle_connection(stream, router).await;
        });
    }
}

async fn handle_connection(mut stream: TcpStream, router: Arc<Router>) {
    let response = match timeout(REQUEST_TIMEOUT, async {
        let request = decode_request(&mut stream).await?;
        Ok::<_, HttpError>(router.handler(request).await)
    })
    .await
    {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => HttpResponse::from(e),
        Err(_) => HttpResponse::from(HttpError::Timeout),
    };

    if let Err(e) = stream.write_all(response.to_string().as_bytes()).await {
        error!(error = %e, "failed to write response");
    }
}
