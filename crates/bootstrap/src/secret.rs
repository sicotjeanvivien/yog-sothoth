//! The two shapes a secret takes: [`SecretKey`] (nothing worth showing) and
//! [`SecretUrl`] (a carrier worth keeping). The secret part is never printable.
//!
//! ⚠️ `new` is crate-private: a downstream `Config` gets a secret only from this
//! crate's readers (`required_secret_*`, `optional_secret_url`, an `Endpoint`).

use std::fmt;

use percent_encoding::percent_decode_str;

/// Placeholder substituted for every secret this module hides.
pub(crate) const REDACTED: &str = "***REDACTED***";

/// What a whole masked value renders as, where [`REDACTED`] marks a removed
/// component; shared by [`SecretKey`] and `Endpoint`'s header rendering.
pub(crate) const MASKED: &str = "****";

/// Shortest secret fragment [`SecretUrl::scrub`] replaces on its own.
/// ⚠️ Below it a fragment (`v2`, `db`) is a coincidence; the whole-URL pass has no floor.
const MIN_SCRUBBABLE: usize = 4;

/// A secret with no carrier worth showing (an API key, a token): `Display` and
/// `Debug` render `****` whatever it holds.
/// ⚠️ Call [`SecretKey::expose`] only where it is consumed, never to log or format an error.
#[derive(Clone)]
pub struct SecretKey(String);

impl SecretKey {
    /// Crate-private, see the module docs.
    pub(crate) fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Return the raw secret. Only at the point of use.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Build one directly, for a test in another crate.
    /// ⚠️ Behind `test-support`, so production code cannot reach it.
    #[cfg(feature = "test-support")]
    pub fn for_tests(raw: impl Into<String>) -> Self {
        Self::new(raw)
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(MASKED)
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ⚠️ Masked in Debug too: a `#[derive(Debug)]` `Config` prints through it.
        write!(f, "SecretKey({MASKED})")
    }
}

/// A URL carrying a secret: `Display` and `Debug` print what `redact` keeps,
/// the carrier legible and the secret parts gone.
/// ⚠️ A bare key has no carrier and would pass through unchanged: use [`SecretKey`].
#[derive(Clone)]
pub struct SecretUrl(String);

impl SecretUrl {
    /// Crate-private, see the module docs.
    pub(crate) fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Return the raw URL. Only at the point of use.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The URL's scheme, lowercased, without exposing the rest; `None` without a scheme.
    /// ⚠️ `Some` only for an RFC 3986 scheme: the first `://` of `host/a://b` is not one.
    pub fn scheme(&self) -> Option<String> {
        let (candidate, _) = self.0.split_once("://")?;

        let mut chars = candidate.chars();
        let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
        let rest_is_scheme =
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));

        (starts_with_letter && rest_is_scheme).then(|| candidate.to_ascii_lowercase())
    }

    /// Build one directly, for a test in another crate, behind `test-support`.
    #[cfg(feature = "test-support")]
    pub fn for_tests(raw: impl Into<String>) -> Self {
        Self::new(raw)
    }

    /// The URL without its password, and the password percent-decoded, still wrapped:
    /// libpq takes it from `PGPASSWORD`, not from a command line `/proc` shows.
    /// `None` when the value does not parse; no password gives `(url, None)`.
    pub fn split_password(&self) -> Option<(String, Option<SecretKey>)> {
        let mut parsed = url::Url::parse(&self.0).ok()?;
        let password = parsed
            .password()
            .map(|p| SecretKey::new(percent_decode_str(p).decode_utf8_lossy()));
        if password.is_some() {
            parsed.set_password(None).ok()?;
        }
        Some((parsed.to_string(), password))
    }

    /// Remove this URL's secrets from a string **somebody else built**, such as a
    /// `reqwest` error quoting the URL: the whole URL first, so the host survives,
    /// then each secret part, for a URL the third party reformatted.
    pub fn scrub(&self, haystack: &str) -> String {
        let mut out = haystack.replace(&self.0, &redact(&self.0));
        for part in self.secret_parts() {
            if part.len() >= MIN_SCRUBBABLE {
                out = out.replace(part, REDACTED);
            }
        }
        out
    }

    /// The substrings of this URL that [`redact`] hides, derived from its output so
    /// the two cannot drift; the whole value when the chunks cannot be aligned.
    fn secret_parts(&self) -> Vec<&str> {
        let redacted = redact(&self.0);
        let mut parts = Vec::new();
        let mut cursor = 0usize;

        for chunk in redacted.split(REDACTED) {
            if chunk.is_empty() {
                continue;
            }
            let Some(offset) = self.0[cursor..].find(chunk) else {
                return vec![&self.0[..]];
            };
            let at = cursor + offset;
            if at > cursor {
                parts.push(&self.0[cursor..at]);
            }
            cursor = at + chunk.len();
        }
        if cursor < self.0.len() {
            parts.push(&self.0[cursor..]);
        }
        parts
    }
}

impl fmt::Display for SecretUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", redact(&self.0))
    }
}

impl fmt::Debug for SecretUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ⚠️ Masked in Debug too: tracing macros and error chains print through it.
        write!(f, "SecretUrl({})", redact(&self.0))
    }
}

/// Redact the password, path, query and fragment, in that order; a userinfo
/// without a password goes too, unless Postgres.
/// ⚠️ The path goes for every scheme but Postgres: an unknown provider is covered by default.
pub(crate) fn redact(url: &str) -> String {
    redact_fragment(&redact_query(&redact_path(&redact_password(url))))
}

/// Postgres, whose path (the database) and role are ours and stay visible.
fn is_postgres(url: &str) -> bool {
    url.starts_with("postgres://") || url.starts_with("postgresql://")
}

/// Replace the password of `scheme://user:password@host`, up to the **last** `@` of
/// the authority; the role stays.
/// ⚠️ An `@` past the authority is ambiguous: with a `:`, all but the scheme goes;
/// without one, the userinfo is printed, bare token included.
pub(crate) fn redact_password(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let start = scheme_end + "://".len();
    let rest = &url[start..];

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let at_beyond_authority = rest[authority_end..].contains('@');

    match authority.rfind('@') {
        // Well-formed: the userinfo opens and closes inside the authority.
        Some(at) if !at_beyond_authority => {
            let userinfo = &authority[..at];
            let Some(colon) = userinfo.find(':') else {
                // ⚠️ No password: a Postgres role stays, any other userinfo may be a
                // token (`https://<token>@host`) and goes.
                return if is_postgres(url) {
                    url.to_string()
                } else {
                    format!("{}{}{}", &url[..start], REDACTED, &url[start + at..])
                };
            };
            format!(
                "{}{}:{}{}",
                &url[..start],
                &userinfo[..colon],
                REDACTED,
                &url[start + at..]
            )
        }
        // Ambiguous, and a password could be in there. Fail closed.
        _ if at_beyond_authority && authority.contains(':') => {
            format!("{}{}", &url[..start], REDACTED)
        }
        // No userinfo, or one that cannot hold a password.
        _ => url.to_string(),
    }
}

/// Replace the path with the placeholder, unless the scheme is Postgres; a path of
/// separators only is left alone.
/// ⚠️ Runs after [`redact_password`]: a `/` in a password is gone by then.
fn redact_path(url: &str) -> String {
    // Postgres keeps its path: it is the database name.
    if is_postgres(url) {
        return url.to_string();
    }
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let authority_start = scheme_end + "://".len();
    let Some(offset) = url[authority_start..].find('/') else {
        return url.to_string();
    };
    let path_start = authority_start + offset;
    let path_end = url[path_start..]
        .find(['?', '#'])
        .map_or(url.len(), |i| path_start + i);

    if url[path_start..path_end].trim_matches('/').is_empty() {
        return url.to_string();
    }
    format!("{}/{}{}", &url[..path_start], REDACTED, &url[path_end..])
}

/// Replace the `?query_string` portion with `?***REDACTED***`.
fn redact_query(url: &str) -> String {
    match url.find('?') {
        Some(idx) => format!("{}?{}", &url[..idx], REDACTED),
        None => url.to_string(),
    }
}

/// Replace the `#fragment` with `#***REDACTED***`, whatever the scheme.
pub(crate) fn redact_fragment(url: &str) -> String {
    match url.find('#') {
        Some(idx) => format!("{}#{}", &url[..idx], REDACTED),
        None => url.to_string(),
    }
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
