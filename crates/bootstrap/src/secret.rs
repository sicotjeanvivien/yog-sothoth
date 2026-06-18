use std::fmt;

/// URL that may carry a secret in its query string (e.g. `?api-key=...`).
///
/// The `Display` and `Debug` impls **redact** the query portion to
/// prevent leaks through logs or error chains. To obtain the raw URL —
/// for example to pass to an HTTP client or a WebSocket — call
/// [`SecretUrl::expose`] explicitly.
///
/// The redaction is intentionally crude: everything after `?` is
/// replaced wholesale. Refining this (preserving non-sensitive
/// parameters) would require a real URL parser; not worth it until a
/// concrete need appears.
#[derive(Clone)]
pub struct SecretUrl(String);

impl SecretUrl {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Return the raw URL. Should only be called at the moment of
    /// consumption (constructing an HTTP client, opening a WebSocket,
    /// etc.) — never for logging or error formatting.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", redact(&self.0))
    }
}

impl fmt::Debug for SecretUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Same treatment in Debug — essential because `{:?}` is
        // commonly used in tracing macros and error chains.
        write!(f, "SecretUrl({})", redact(&self.0))
    }
}

/// Replace the `?query_string` portion with `?***REDACTED***`.
fn redact(url: &str) -> String {
    match url.find('?') {
        Some(idx) => format!("{}?***REDACTED***", &url[..idx]),
        None => url.to_string(),
    }
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
