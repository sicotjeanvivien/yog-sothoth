//! The client a credential names, or why it names none.

use yog_core::domain::{ApiClient, ApiKey};

use crate::application::{KeyRing, LiveScope, LiveTokens, Unauthorized};

/// The client of the `Authorization` value `credential`.
pub(crate) fn identify(credential: &str, ring: &KeyRing) -> Result<ApiClient, Unauthorized> {
    let token = bearer(credential).ok_or(Unauthorized::Scheme)?;
    let key = ApiKey::parse(token).map_err(|_| Unauthorized::Malformed)?;
    ring.client_of(&key).ok_or(Unauthorized::Unknown)
}

/// The client of the `Authorization` value `credential`, opening `asked` on the live stream.
/// ⚠️ An API key is refused here: the live stream opens with a live token only.
pub(crate) fn identify_live(
    credential: &str,
    asked: &LiveScope,
    tokens: &LiveTokens,
    now: i64,
) -> Result<String, Unauthorized> {
    let token = bearer(credential).ok_or(Unauthorized::Scheme)?;
    if ApiKey::parse(token).is_ok() {
        return Err(Unauthorized::KeyOnLive);
    }
    tokens.open(token, asked, now)
}

/// The token of a `Bearer` credential, empty when it carries none.
/// ⚠️ The scheme is case-insensitive (RFC 9110 §11.1).
fn bearer(value: &str) -> Option<&str> {
    let value = value.trim();
    let (scheme, token) = value
        .split_once(|c: char| c.is_ascii_whitespace())
        .unwrap_or((value, ""));
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
