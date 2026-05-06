use crate::bootstrap::{self, Container};
use crate::interface::{HttpError, HttpResponse, Router, decode_request};
use anyhow::Context;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::{Duration, timeout},
};
use tracing::{error, info};

pub(crate) struct Server {
    pub(crate) router: Arc<Router>,
    pub(crate) tcp_listener: TcpListener,
    pub(crate) limiter: Arc<Semaphore>,
}

impl Server {
    pub(crate) async fn init() -> anyhow::Result<Self> {
        let container = Container::build().await;
        let router = bootstrap::build_router(&container).await;
        let limiter = Arc::new(Semaphore::new(100));
        let app_url = env::var("APP_URL").context("APP_URL must be set in .env")?;
        let tcp_listener = TcpListener::bind(&app_url)
            .await
            .context("app_url is invalid ")?;
        info!("Server starting on {}", app_url);
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
                        error!(error = %e, "Accept error");
                    }
                }
            }
        };

        tokio::select! {
            _ = server => {}
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received — stopping server");
            }
        }

        info!("Server stopped");
    }

    async fn handle_accept(&self, stream: TcpStream, addr: SocketAddr) {
        info!(client = %addr, "New connection");
        let router = Arc::clone(&self.router);
        let permit = match self.limiter.clone().acquire_owned().await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to acquire permit: {:?}", e);
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
    let response = match timeout(Duration::from_secs(5), async {
        let request = decode_request(&mut stream).await?;
        Ok::<_, HttpError>(router.handler(request).await)
    })
    .await
    {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => HttpResponse::from(e),
        Err(_) => HttpResponse::from(HttpError::Timeout),
    };
    if let Err(e) = stream.write_all(&response.to_string().as_bytes()).await {
        error!("Failed to write response: {}", e);
    }
}
