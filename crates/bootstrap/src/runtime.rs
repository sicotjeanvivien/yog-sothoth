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
    // Both reads are trimmed, and `RUST_LOG` is the one that bites hardest.
    // `try_from_default_env` reads it raw, so a `\r` from the CRLF `.env`
    // sourced into the shell turns `info` into the directive `info\r` —
    // parsed as a *target* name rather than a level, matching nothing.
    // Measured 2 September 2026 on this binary: `RUST_LOG=info` emits its
    // startup lines, `RUST_LOG=info\r` emits **none at all**. A daemon that
    // silently stops logging is the worst of the failure modes here, and it
    // was one line below a comment congratulating itself for avoiding a
    // milder one on `LOG_FORMAT`.
    let format = std::env::var("LOG_FORMAT").unwrap_or_default();
    let format = format.trim();

    let filter = build_filter(std::env::var(EnvFilter::DEFAULT_ENV).ok().as_deref());

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

/// Build the log filter from a raw `RUST_LOG` value.
///
/// Split out of `init_tracing` so it can be tested: the subscriber it feeds
/// is global and installed once per process, which a test cannot exercise
/// twice. Absent, blank, or unparseable all fall back to `info` — the same
/// fallback the caller had before, kept deliberately silent because logging
/// is not yet up to report on itself.
fn build_filter(raw: Option<&str>) -> EnvFilter {
    raw.map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(keep_shutdown_audible)
        .and_then(|raw| EnvFilter::try_new(raw).ok())
        .unwrap_or_else(|| EnvFilter::new("info"))
}

/// The target every daemon's stop speaks under.
///
/// `shutdown.rs` holds the stop for all of them, so its lines carry this
/// target rather than the binary's own — see that module's header.
const SHUTDOWN_TARGET: &str = "yog_bootstrap";

/// Ensure a `RUST_LOG` of per-target directives cannot silence the stop.
///
/// ⚠️ **The loss this closes is silent, which is why documenting it was not
/// enough.** `EnvFilter` has no implicit global level: a value made only of
/// directives prints *nothing* for a target it does not name. On 14 September
/// 2026 the stop moved to `yog_bootstrap::shutdown`, and this repository's own
/// `RUST_LOG` — six per-crate directives, no bare level — stopped printing
/// every line of it, the `warn!` naming a stage destroyed mid-write included. A
/// torn stop then reads exactly like a clean one, and ten measured cycles said
/// so before the filter was suspected.
///
/// It only ever *adds*, and only when the operator has said nothing that
/// covers the target: a bare level anywhere in the value already covers it, and
/// so does naming `yog_bootstrap` — at any level, silencing it included, which
/// stays possible on purpose.
fn keep_shutdown_audible(raw: &str) -> String {
    let already_covered = raw
        .split(',')
        .map(str::trim)
        .any(|directive| !directive.contains('=') || directive.starts_with(SHUTDOWN_TARGET));

    if already_covered {
        raw.to_owned()
    } else {
        format!("{SHUTDOWN_TARGET}=info,{raw}")
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
