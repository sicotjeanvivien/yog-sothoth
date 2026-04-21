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
        let end = after
            .find(|c: char| c == '&' || c == ')' || c == ' ' || c == '"')
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_single_api_key_in_url() {
        let input = "https://example.com/?api-key=abc123";
        assert_eq!(
            redact_api_key(input),
            "https://example.com/?api-key=***REDACTED***"
        );
    }

    #[test]
    fn redact_api_key_followed_by_ampersand() {
        let input = "https://example.com/?api-key=abc123&foo=bar";
        assert_eq!(
            redact_api_key(input),
            "https://example.com/?api-key=***REDACTED***&foo=bar"
        );
    }

    #[test]
    fn redact_api_key_inside_parentheses() {
        let input = "HTTP error for url (https://example.com/?api-key=abc123)";
        assert_eq!(
            redact_api_key(input),
            "HTTP error for url (https://example.com/?api-key=***REDACTED***)"
        );
    }

    #[test]
    fn redact_api_key_idempotent() {
        let already_redacted = "https://example.com/?api-key=***REDACTED***";
        assert_eq!(redact_api_key(already_redacted), already_redacted);
    }

    #[test]
    fn redact_no_api_key_unchanged() {
        let input = "no secret here";
        assert_eq!(redact_api_key(input), input);
    }

    #[test]
    fn redact_multiple_api_keys() {
        let input = "first url ?api-key=aaa and second ?api-key=bbb done";
        assert_eq!(
            redact_api_key(input),
            "first url ?api-key=***REDACTED*** and second ?api-key=***REDACTED*** done"
        );
    }
}