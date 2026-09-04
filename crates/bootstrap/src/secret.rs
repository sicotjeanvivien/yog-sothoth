//! The two shapes a secret takes in this workspace, and the one rule
//! they share: **the secret part is never printable; only the carrier is.**
//!
//! Two types rather than one, because the carriers differ. A bare key has
//! nothing worth showing, so [`SecretKey`] shows nothing. A connection string
//! has a host, a port, a database and a role — and redacting those away costs
//! the only diagnostic a crashing daemon leaves behind, which this repository
//! learned the expensive way (see `04 - release/cle-api-en-clair-dans-les-logs.md`
//! in the tracking repo: "retirer l'URL avait emporté le seul diagnostic qui
//! restait"). So [`SecretUrl`] keeps the carrier and redacts the two places a
//! secret hides in a URL.
//!
//! # Why `new` is not public
//!
//! Neither type can be built from outside this crate. A downstream `Config`
//! obtains one through [`crate::required_secret_url`] or
//! [`crate::required_secret_key`], and by no other route — so "a secret is
//! wrapped" is enforced by the compiler rather than by remembering to do it at
//! every site. The previous shape of this module made it a convention, and the
//! convention held at seven sites out of nine.

use std::fmt;

/// Placeholder substituted for every secret this module hides.
const REDACTED: &str = "***REDACTED***";

/// A secret that carries nothing worth showing — an API key, a token.
///
/// `Display` and `Debug` render `****` **unconditionally**: this type knows
/// nothing about the shape of what it holds, which is the point. A redactor
/// that has to recognise a shape is a redactor that misses the next one — the
/// reasoning is the same one that kept `redact_api_key` out of this crate.
///
/// Call [`SecretKey::expose`] at the moment of consumption — building a
/// request header, opening a connection — and never to log or to format an
/// error.
#[derive(Clone)]
pub struct SecretKey(String);

impl SecretKey {
    /// Not public on purpose — see the module docs. Downstream crates get a
    /// `SecretKey` from [`crate::required_secret_key`].
    pub(crate) fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Return the raw secret. Only at the point of use.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Build one directly, for a test in another crate that needs a secret
    /// without an environment to read it from.
    ///
    /// Behind `test-support` so that **production code cannot reach it**: the
    /// only route there is [`crate::required_secret_key`], and that is the
    /// whole point of keeping `new` private.
    #[cfg(feature = "test-support")]
    pub fn for_tests(raw: impl Into<String>) -> Self {
        Self::new(raw)
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("****")
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Same treatment in Debug — essential because `{:?}` is what
        // `#[derive(Debug)]` on a `Config` reaches for, and two of this
        // workspace's four configs derive it.
        f.write_str("SecretKey(****)")
    }
}

/// A URL that carries a secret, with the non-secret part kept legible.
///
/// `Display` and `Debug` redact **the two places a URL hides a secret**, and
/// only those:
///
/// - the password in the userinfo — `postgres://role:pass@host/db`;
/// - the query string — `https://host/?api-key=…`.
///
/// Everything else survives: scheme, role, host, port, path. That is
/// deliberate. A daemon that dies on startup must still say *which* database
/// or *which* provider it could not reach.
///
/// ```text
/// postgresql://yog_indexer:hunter2@localhost:5433/yog_sothoth
///   → postgresql://yog_indexer:***REDACTED***@localhost:5433/yog_sothoth
/// https://mainnet.helius-rpc.com/?api-key=abc123
///   → https://mainnet.helius-rpc.com/?***REDACTED***
/// ```
///
/// # What this type does not cover
///
/// A value that is *only* a secret — a bare API key or token — has no carrier
/// to preserve, and this type would return it unchanged. That is what
/// [`SecretKey`] is for. Reaching for `SecretUrl` because it was the one that
/// existed is exactly how `JUPITER_API_KEY` came to be stored in a type that
/// could not redact it.
///
/// Call [`SecretUrl::expose`] at the moment of consumption — constructing an
/// HTTP client, opening a WebSocket, connecting a pool — never for logging or
/// error formatting.
#[derive(Clone)]
pub struct SecretUrl(String);

impl SecretUrl {
    /// Not public on purpose — see the module docs. Downstream crates get a
    /// `SecretUrl` from [`crate::required_secret_url`].
    pub(crate) fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Return the raw URL. Only at the point of use.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Build one directly, for a test in another crate. Behind `test-support`
    /// for the reason spelled out on [`SecretKey::for_tests`].
    #[cfg(feature = "test-support")]
    pub fn for_tests(raw: impl Into<String>) -> Self {
        Self::new(raw)
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

/// Redact everything a URL can use to carry a credential, keeping the rest.
///
/// Three places, applied in that order: the password in the userinfo, the path,
/// and the query string. Deliberately crude, and deliberately without a
/// URL-parser dependency.
///
/// # The rule fails closed, and that is the point
///
/// The path is redacted for **every scheme except Postgres**, whose path is the
/// database name and is the diagnostic this type exists to preserve. So a
/// provider nobody has met yet — one that puts its key in a path segment, as
/// Alchemy (`/v2/<key>`) and QuickNode (`/<token>/`) both do — is covered by
/// default rather than by having been recognised. An earlier shape of this
/// function knew only about `?`, and that is exactly how it came to return a
/// bare API key in the clear.
///
/// What survives is scheme, userinfo role, host and port: enough to name which
/// provider or which database a dying process could not reach, which is the
/// whole reason this is not a blanket `****`.
fn redact(url: &str) -> String {
    redact_query(&redact_path(&redact_password(url)))
}

/// Replace the password in `scheme://user:password@host` with the placeholder.
///
/// The password is what follows the first `:` of the userinfo, up to the
/// **last** `@` — last, because a password may legitimately contain `@` while a
/// hostname may not. The role stays visible: it names which of the five
/// least-privilege Postgres roles was in play.
///
/// The `@` is normally inside the authority, which ends at the first `/`, `?`
/// or `#`. When it is not, an unencoded `/` sits inside the password and has
/// pushed it past that bound; the search then widens to the whole pre-query
/// remainder rather than giving up, because giving up here returns the password
/// in the clear on a URL that looks perfectly ordinary.
fn redact_password(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let start = scheme_end + "://".len();
    let rest = &url[start..];

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let search_end = if rest[..authority_end].contains('@') {
        authority_end
    } else {
        rest.find(['?', '#']).unwrap_or(rest.len())
    };

    // No `@` means no userinfo, so no password to hide.
    let Some(at) = rest[..search_end].rfind('@') else {
        return url.to_string();
    };
    let userinfo = &rest[..at];
    let Some(colon) = userinfo.find(':') else {
        // `user@host` — a username alone is not a secret.
        return url.to_string();
    };

    format!(
        "{}{}:{}{}",
        &url[..start],
        &userinfo[..colon],
        REDACTED,
        &url[start + at..]
    )
}

/// Replace the path with the placeholder, unless the scheme is Postgres.
///
/// Runs *after* [`redact_password`], so a password containing `/` has already
/// become the placeholder and cannot be mistaken for the start of a path.
///
/// A path made only of separators — `https://host/` — hides nothing and is left
/// alone, so that the far more common "key in the query string" URL keeps its
/// trailing slash and stays recognisable.
fn redact_path(url: &str) -> String {
    // Postgres keeps its path: it is the database name.
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
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

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
