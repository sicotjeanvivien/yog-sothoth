//! A 429 read the same way for every provider: a [`SourceError::RateLimited`], counted
//! `rate_limited`, before `error_for_status` turns it into a plain HTTP error.

use std::time::Duration;

use crate::error::SourceError;

/// `RateLimited` on a 429, with its `Retry-After`; any other response as it came.
pub(super) fn refused(response: reqwest::Response) -> Result<reqwest::Response, SourceError> {
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited {
            retry_after: parse_retry_after(response.headers()),
        });
    }
    Ok(response)
}

/// `Retry-After` in its delta-seconds form; the HTTP-date form yields `None`.
fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

#[cfg(test)]
#[path = "rate_limit_tests.rs"]
mod tests;
