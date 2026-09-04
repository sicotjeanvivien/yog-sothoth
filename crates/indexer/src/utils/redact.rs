//! Redaction of secrets in log messages and error strings.
//!
//! The goal is to prevent API keys from leaking into logs when underlying
//! libraries (reqwest, sqlx, solana-client) include the full URL in their
//! error messages. Applied at the logging call site, not in the error type
//! itself — we keep the raw error for debugging and only redact on output.

/// Redact `api-key=...` parameters in a string.
///
/// Covers the Helius format (`?api-key=...`), which is what `SOLANA_RPC_WS`
/// and `SOLANA_RPC_HTTP` hold today. Extend this function if adding a provider
/// with a different parameter name (QuickNode: `token=`, Triton: `auth=`).
///
/// # ⚠️ It does not cover a credential in the URL *path*, and that gap is real
///
/// Alchemy authenticates with `/v2/<key>` and QuickNode with `/<token>/`, and
/// the repository's own provider study puts Alchemy first. This function
/// matches the literal `api-key=` and nothing else, so the day either variable
/// points at such an endpoint, a `PubsubClient::new` or `RpcClient` failure —
/// whose `Display` carries the full URL — walks straight through it and into a
/// `warn!`. Nothing leaks on the current configuration; the trigger is the
/// provider migration, not a code change.
///
/// `SecretUrl` learned about path credentials in September 2026; this function
/// did not, and teaching it would only add a fourth shape to a redactor whose
/// whole weakness is that it must recognise shapes. The structural answer is
/// the one `yog-context` took at its boundary (`error/source.rs`, `without_url`)
/// — or, better here where the error types are third-party, scrubbing the
/// **known** configured secret rather than a pattern that looks like one. Both
/// are a design change, not an edit to this function.
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
