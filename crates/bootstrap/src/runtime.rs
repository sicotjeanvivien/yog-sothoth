use tracing_subscriber::EnvFilter;

/// Install the rustls crypto provider.
///
/// rustls 0.23 removed the implicit crypto provider selection — without
/// this call, the first TLS handshake (e.g. the WebSocket to Helius, an
/// HTTPS RPC request) panics. Each binary that performs TLS must call
/// this exactly once, **before** any TLS connection is established.
///
/// Panics on failure: this is a process-level invariant, and no recovery
/// is meaningful if the crypto provider cannot be installed.
pub fn init_rustls() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");
}

/// Initialise the global tracing subscriber.
///
/// Format is selected from the `LOG_FORMAT` environment variable:
///   - `json` → machine-readable, suitable for log collectors
///     (Loki, Datadog, …).
///   - anything else → human-readable text, suitable for local
///     development.
///
/// Log level is controlled by `RUST_LOG` (defaults to `info`):
///
/// ```text
/// RUST_LOG=yog_indexer=debug,yog_core=debug,warn
/// ```
///
/// Idempotent in the sense that subsequent calls are silently ignored
/// by `tracing`. Each binary should call this once at the top of `main`,
/// after `init_rustls` and before any code that emits logs.
pub fn init_tracing() {
    let format = std::env::var("LOG_FORMAT").unwrap_or_default();
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
