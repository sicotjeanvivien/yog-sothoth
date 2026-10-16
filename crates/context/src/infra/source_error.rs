//! A `reqwest` failure turned into a [`SourceError`], with its URL stripped; it sits with the
//! provider clients, so the ports never depend on the HTTP client.

use crate::error::SourceError;

/// Convert a `reqwest` failure, **stripping the URL first**: reqwest's errors may carry the full
/// URL, and the Helius key rides in its query string.
/// ⚠️ At the boundary, not at the log sites: nothing out of here carries a URL, so none can leak.
impl From<reqwest::Error> for SourceError {
    fn from(e: reqwest::Error) -> Self {
        // Read the kind before `without_url`, which consumes the error: `.json::<T>()` failures
        // decode, everything else (connect, timeout, non-2xx) is transport.
        let is_decode = e.is_decode();
        let redacted = e.without_url();

        // ⚠️ Without its URL, reqwest's `Display` renders every transport failure alike: the cause
        // chain, which carries no secret, is appended to tell them apart.
        let mut message = redacted.to_string();
        let mut cause = std::error::Error::source(&redacted);
        while let Some(current) = cause {
            message.push_str(": ");
            message.push_str(&current.to_string());
            cause = current.source();
        }

        if is_decode {
            Self::Decode(message)
        } else {
            Self::Http(message)
        }
    }
}

#[cfg(test)]
#[path = "source_error_tests.rs"]
mod tests;
