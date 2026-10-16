use std::str::FromStr;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// Install the rustls crypto provider; panics on failure.
/// ⚠️ Once per binary, before the first TLS connection: rustls picks no provider
/// itself, and the first handshake would panic.
pub fn init_rustls() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");
}

/// Install the global tracing subscriber: JSON when `LOG_FORMAT=json`, text
/// otherwise, filtered by `RUST_LOG` (default `info`). Once, at the top of `main`.
pub fn init_tracing() {
    // ⚠️ Both reads are trimmed: a CRLF `\r` turns `RUST_LOG=info` into a target
    // matching nothing, and the daemon logs nothing at all.
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

/// Build the log filter from a raw `RUST_LOG`; absent, blank or unparseable is
/// `info`, silently, since logging is not up yet to report on itself.
fn build_filter(raw: Option<&str>) -> EnvFilter {
    raw.map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(keep_shutdown_audible)
        .and_then(|raw| EnvFilter::try_new(raw).ok())
        .unwrap_or_else(|| EnvFilter::new("info"))
}

/// The target every daemon's stop logs under, `shutdown.rs` being shared.
const SHUTDOWN_TARGET: &str = "yog_bootstrap";

/// Add `yog_bootstrap=info` to a `RUST_LOG` that neither sets a bare level nor
/// names `yog_bootstrap`: per-target directives alone print nothing else.
/// ⚠️ Without it a torn stop reads like a clean one; naming the target, at any level, is respected.
fn keep_shutdown_audible(raw: &str) -> String {
    let already_covered = raw
        .split(',')
        .map(str::trim)
        .any(|directive| is_bare_level(directive) || targets_shutdown(directive));

    if already_covered {
        raw.to_owned()
    } else {
        format!("{SHUTDOWN_TARGET}=info,{raw}")
    }
}

/// Does this directive set the level for *everything*?
/// ⚠️ Not "has no `=`": a bare target means TRACE for it alone. And an empty
/// segment (a trailing comma) parses as `error` but is no level.
fn is_bare_level(directive: &str) -> bool {
    !directive.is_empty() && LevelFilter::from_str(directive).is_ok()
}

/// Does this directive name the stop's target, exactly rather than by prefix?
fn targets_shutdown(directive: &str) -> bool {
    directive
        .split(['=', '['])
        .next()
        .is_some_and(|target| target.trim() == SHUTDOWN_TARGET)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
