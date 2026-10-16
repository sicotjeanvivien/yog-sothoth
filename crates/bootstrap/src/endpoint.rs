//! An external endpoint: `<FUNCTION>_URL`, `<FUNCTION>_KEY` and an optional
//! `<FUNCTION>_HEADER`; the key is substituted for `{key}` wherever the operator
//! wrote it, so the code knows no provider's convention.
//! ⚠️ A credential in a header has no counterpart to [`SecretUrl::scrub`]: a client
//! echoing request metadata in its errors would print it.

use std::fmt;

use crate::secret::{
    MASKED, REDACTED, SecretKey, SecretUrl, redact, redact_fragment, redact_password,
};

/// The placeholder an operator writes where the credential belongs.
pub(crate) const KEY_PLACEHOLDER: &str = "{key}";

/// An endpoint's address, and the credential it substitutes into it.
/// ⚠️ Built only by [`crate::required_endpoint`], like [`SecretUrl`].
#[derive(Clone)]
pub struct Endpoint {
    /// The URL as written, `{key}` included and the credential absent.
    template: String,
    /// The header carrying the credential, name and value template as written;
    /// `None` when the credential rides in the URL, or there is none.
    header: Option<(String, String)>,
    /// `None` for a public endpoint — one that has no credential to carry.
    key: Option<SecretKey>,
}

impl Endpoint {
    /// Crate-private, see [`crate::required_endpoint`].
    pub(crate) fn new(
        template: String,
        header: Option<(String, String)>,
        key: Option<SecretKey>,
    ) -> Self {
        Self {
            template,
            header,
            key,
        }
    }

    /// The address to call, credential substituted in.
    /// ⚠️ A [`SecretUrl`], never a `String`: clients copy it into their errors, which
    /// [`SecretUrl::scrub`] cleans.
    pub fn url(&self) -> SecretUrl {
        match &self.key {
            Some(key) => SecretUrl::new(self.template.replace(KEY_PLACEHOLDER, key.expose())),
            None => SecretUrl::new(self.template.clone()),
        }
    }

    /// Build one directly, for a test in another crate, behind `test-support`.
    #[cfg(feature = "test-support")]
    pub fn for_tests(template: impl Into<String>, key: Option<&str>) -> Self {
        Self::new(template.into(), None, key.map(SecretKey::new))
    }

    /// The header to send, credential substituted in: the name bare, the value a
    /// [`SecretKey`], since it *is* the credential. `None` without a header.
    pub fn header(&self) -> Option<(&str, SecretKey)> {
        self.header.as_ref().map(|(name, value)| {
            let assembled = match &self.key {
                Some(key) => value.replace(KEY_PLACEHOLDER, key.expose()),
                None => value.clone(),
            };
            (name.as_str(), SecretKey::new(assembled))
        })
    }

    /// What [`fmt::Display`] prints: with `{key}`, the template minus userinfo and
    /// fragment; without, [`redact`], since it may hold an inlined key.
    /// ⚠️ A second credential inlined beside `{key}`, in the path or query, is printed.
    fn displayed(&self) -> String {
        if self.template.contains(KEY_PLACEHOLDER) {
            redact_fragment(&redact_password(&self.template))
        } else {
            redact(&self.template)
        }
    }

    /// What the header half prints: a value with `{key}` as written, any other
    /// masked; the name never is.
    /// ⚠️ A key inlined beside `{key}` (`legacy-abc123 {key}`) is printed whole.
    fn displayed_header(&self) -> Option<String> {
        self.header.as_ref().map(|(name, value)| {
            if value.contains(KEY_PLACEHOLDER) {
                format!("{name}: {value}")
            } else {
                format!("{name}: {MASKED}")
            }
        })
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.displayed())?;
        // Which header is set is the diagnostic when a provider expects another.
        match self.displayed_header() {
            Some(header) => write!(f, " [{header}]"),
            None => Ok(()),
        }
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ⚠️ Masked in Debug too: a `#[derive(Debug)]` `Config` prints through it.
        write!(
            f,
            "Endpoint({}, header: {}, key: {})",
            self.displayed(),
            self.displayed_header().as_deref().unwrap_or("none"),
            if self.key.is_some() { REDACTED } else { "none" }
        )
    }
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
