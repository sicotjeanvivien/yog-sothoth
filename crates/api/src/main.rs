use dotenvy::dotenv;
use tracing_subscriber::EnvFilter;

use crate::bootstrap::Server;

mod bootstrap;
mod interface;

// ── Logging ──────────────────────────────────────────────────────────────────

/// Initializes the tracing subscriber.
///
/// Format is selected from the `LOG_FORMAT` environment variable:
/// - `json` → machine-readable, suitable for log collectors (Loki, Datadog…)
/// - anything else → human-readable text, suitable for local development
///
/// Log level is controlled by `RUST_LOG` (defaults to `info`):
/// ```text
/// RUST_LOG=yog_indexer=debug,yog_core=debug,warn
/// ```
fn init_tracing() {
    let format = std::env::var("LOG_FORMAT").unwrap_or_default();

    // Respect RUST_LOG
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if format.eq_ignore_ascii_case("json") {
        tracing_subscriber::fmt()
            .json()
            .with_current_span(true)
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_target(true)
            .with_env_filter(filter)
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Must be first: rustls 0.23 requires an explicit crypto provider.
    // Without this, any TLS connection (WS to Helius, HTTPS to the RPC)
    // panics on the first handshake.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    dotenv().ok();
    init_tracing();

    let server: Server = Server::init().await?;
    server.run().await;
    Ok(())
}
