//! Redaction of secrets in log messages and error strings.
//!
//! The goal is to prevent API keys from leaking into logs when underlying
//! libraries (reqwest, sqlx, solana-client) include the full URL in their
//! error messages. Applied at the logging call site, not in the error type
//! itself — we keep the raw error for debugging and only redact on output.

/// Redact `api-key=...` parameters in a string.
///
/// Covers the Helius format (`?api-key=...`). Extend this function if
/// adding a provider with a different parameter name (QuickNode: `token=`,
/// Triton: `auth=`, etc.).
pub(crate) fn redact_api_key(msg: &str) -> String {
    let mut result = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(start) = rest.find("api-key=") {
        result.push_str(&rest[..start]);
        result.push_str("api-key=***REDACTED***");
        let after = &rest[start + "api-key=".len()..];
        let end = after.find(['&', ')', ' ', '"']).unwrap_or(after.len());
        rest = &after[end..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
#[path = "redact_tests.rs"]
mod tests;
