//! An external endpoint: an address the operator writes, and the credential it
//! needs, held apart.
//!
//! # Why the two halves are separate variables
//!
//! A variable named after the protocol it speaks — `SOLANA_RPC_HTTP` — does not
//! refuse anything: any HTTP call towards a Solana node fits the name, so three
//! roles for two dependencies accumulated under it without contradicting it. A
//! variable named after the **function** it serves refuses on its own: nobody
//! will read token metadata from `POOL_ACCOUNT_URL`. So each endpoint is a
//! `<FUNCTION>_URL` / `<FUNCTION>_KEY` pair, read through
//! [`crate::required_endpoint`], and the day one provider serves the DAS while
//! another serves the accounts, the configuration can already say so.
//!
//! # Why a `{key}` template rather than a list of providers
//!
//! The credential does not attach at the same place twice: Helius appends
//! `?api-key=`, Alchemy `/v2/<key>`, QuickNode `/<token>/`, Triton `?auth=`.
//! Either the code knows every convention — an enum of providers — or the
//! operator writes the shape once, in the URL, and the code knows none. The
//! second is taken: nothing here has to be updated when a new provider appears.
//!
//! That is the same choice `redact_api_key` lost, and it lands the other way
//! **for a reason worth naming**: knowing a shape in order to *build* is safe,
//! knowing a shape in order to *redact* is not. A wrong template gives a frank
//! 401 on the first call; a wrong redactor writes a secret in the clear and
//! nobody sees it.
//!
//! # Why the credential does not always live in the URL
//!
//! The first shape of this module assumed it did. Measured 8 September 2026,
//! across the providers this workspace may actually reach, it does not:
//!
//! - a self-hosted Yellowstone server ships `"x_token": null` and takes no
//!   credential at all — and carries a pluggable `auth` block beside it, so
//!   even upstream does not fix one scheme;
//! - QuickNode, Alchemy, Shyft and Helius authenticate a gRPC stream with a
//!   **metadata header**, and the header's *name* is itself a provider
//!   convention (`x-token` for most, but nothing makes that universal);
//! - Triton's load balancers accept `user:password` basic auth **in the URL**,
//!   where the password is the token.
//!
//! So the placeholder is not bound to the URL: it is looked for in the URL
//! **and** in an optional `<FUNCTION>_HEADER` template, and substituted
//! wherever the operator put it. This is the same bet as above rather than a
//! second mechanism — one placeholder, two possible carriers, and no provider
//! known to the code. The refusal rule generalises with it: a `_KEY` is
//! refused when there is no `{key}` **anywhere** to receive it, not merely
//! none in the URL.

use std::fmt;

use crate::secret::{
    MASKED, REDACTED, SecretKey, SecretUrl, redact, redact_fragment, redact_password,
};

/// The placeholder an operator writes where the credential belongs.
pub(crate) const KEY_PLACEHOLDER: &str = "{key}";

/// An endpoint's address, and the credential it substitutes into it.
///
/// Built only by [`crate::required_endpoint`], for the reason [`SecretUrl`] is:
/// so that "the key never sits in the address" is what the type is, not what
/// every `Config` remembers to do.
#[derive(Clone)]
pub struct Endpoint {
    /// The URL as written, `{key}` included and the credential absent.
    template: String,
    /// The metadata/HTTP header carrying the credential, split into its name
    /// and its value template, both as the operator wrote them. `None` when the
    /// credential rides in the URL, or when there is none.
    header: Option<(String, String)>,
    /// `None` for a public endpoint — one that has no credential to carry.
    key: Option<SecretKey>,
}

impl Endpoint {
    /// Not public on purpose — see [`crate::required_endpoint`].
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

    /// The address to actually call, credential substituted in.
    ///
    /// A [`SecretUrl`] and not a `String`: from here on the value *does* carry
    /// the key, and it travels to `reqwest` and `solana-client`, which copy the
    /// URL they were given into their own error messages. Those errors reach a
    /// `warn!` as plain strings that never passed through this type — which is
    /// what [`SecretUrl::scrub`] exists for, and why the assembled form keeps
    /// the type that can do it.
    ///
    /// A template with no `{key}` is returned as written: a public endpoint has
    /// nothing to substitute, and wrapping it costs nothing.
    pub fn url(&self) -> SecretUrl {
        match &self.key {
            Some(key) => SecretUrl::new(self.template.replace(KEY_PLACEHOLDER, key.expose())),
            None => SecretUrl::new(self.template.clone()),
        }
    }

    /// Build one directly, for a test in another crate that has no environment
    /// to read it from. Behind `test-support` for the reason spelled out on
    /// [`SecretKey::for_tests`].
    #[cfg(feature = "test-support")]
    pub fn for_tests(template: impl Into<String>, key: Option<&str>) -> Self {
        Self::new(template.into(), None, key.map(SecretKey::new))
    }

    /// The same, with a header carrying the credential.
    ///
    /// Separate from [`Endpoint::for_tests`] rather than a fourth argument on
    /// it: the header case is the rarer one, and every existing caller would
    /// otherwise gain a `None` that says nothing.
    #[cfg(feature = "test-support")]
    pub fn for_tests_with_header(
        template: impl Into<String>,
        header: (&str, &str),
        key: Option<&str>,
    ) -> Self {
        Self::new(
            template.into(),
            Some((header.0.to_string(), header.1.to_string())),
            key.map(SecretKey::new),
        )
    }

    /// The header to send, name and value, credential substituted in.
    ///
    /// The value is a [`SecretKey`] for the reason [`Endpoint::url`] returns a
    /// [`SecretUrl`]: once the substitution has run, the value *is* the
    /// credential — `x-token: <token>` — and must not become a printable
    /// `String` on the way to the client. `SecretKey` masks unconditionally,
    /// which is right here: unlike a URL, a header value has no carrier worth
    /// keeping, so there is nothing to weigh against hiding all of it.
    ///
    /// The **name** is returned bare, because it is not a secret and it is the
    /// diagnostic: a startup log saying which header is being set is what tells
    /// an operator their provider expects a different one.
    ///
    /// `None` when the endpoint carries no header — the credential is in the
    /// URL, or there is none.
    pub fn header(&self) -> Option<(&str, SecretKey)> {
        self.header.as_ref().map(|(name, value)| {
            let assembled = match &self.key {
                Some(key) => value.replace(KEY_PLACEHOLDER, key.expose()),
                None => value.clone(),
            };
            (name.as_str(), SecretKey::new(assembled))
        })
    }

    /// What [`fmt::Display`] prints — the whole address, or a redacted one.
    ///
    /// # This is the fail-closed half of the design
    ///
    /// A template carrying `{key}` is legible in full, and that is the whole
    /// return on separating the two halves: an error that names the endpoint no
    /// longer has to choose between the diagnostic and the leak.
    ///
    /// A template carrying no `{key}` gets no such promise. It is *either* a
    /// public endpoint *or* an operator who pasted the credential straight into
    /// the URL, and **nothing here can tell those apart** — so it falls back on
    /// [`redact`], which keeps scheme, host and port. The cost is a public URL
    /// printed shorter than it needed to be; the alternative cost is a key in
    /// the logs the first time somebody fills the `.env` the old way.
    ///
    /// ⚠️ And a `{key}` is **not** a certificate of safety for the rest of the
    /// URL, which is what an earlier shape of this function assumed. The
    /// placeholder accounts for the one credential the operator externalized;
    /// it says nothing about a second one sitting elsewhere in the same
    /// address — `https://user:s3cret@host/?api-key={key}` is a real shape, and
    /// it was printed whole. So the two components a template has **no reason
    /// to carry** are redacted either way: the userinfo, which no provider here
    /// authenticates through, and the fragment, which nothing in this workspace
    /// reads. What stays legible is what the placeholder is actually about —
    /// scheme, host, port, path and query.
    ///
    /// # Where this stops, and why it stops there
    ///
    /// Path and query stay legible **because that is where the placeholder
    /// lives**: `?api-key={key}` and `/v2/{key}` are the two shapes the whole
    /// design exists to print. So a second credential inlined *beside* the
    /// placeholder — `?api-key={key}&auth=<token>`, or an old key left in a
    /// path segment — is printed too. That is a real boundary, and it is drawn
    /// on purpose rather than half-closed: redacting non-placeholder query
    /// values would cost the legitimate ones (`?commitment=finalized`) and
    /// would still leave the path, which is the same hiding place one step
    /// over. A guard that covers two of three hiding places is the defect this
    /// module was rewritten to remove, not a smaller version of the fix.
    ///
    /// The userinfo and the fragment are redacted precisely because they are
    /// **not** that: no endpoint here authenticates through them and nothing
    /// reads them, so hiding them costs nothing at all. The rule is not "hide
    /// what might be secret" — it is *keep what is a diagnostic, drop what
    /// never is*, which is the same line `redact` draws for Postgres.
    ///
    /// What the operator gets, stated plainly: an address is legible in a log
    /// exactly to the extent that its credentials are in `_KEY` variables. One
    /// left inline is one printed.
    fn displayed(&self) -> String {
        if self.template.contains(KEY_PLACEHOLDER) {
            redact_fragment(&redact_password(&self.template))
        } else {
            redact(&self.template)
        }
    }

    /// What the header half prints, under the **same** fail-closed rule.
    ///
    /// A value template carrying `{key}` prints as written — `x-token: {key}`
    /// tells an operator which header is set and that its credential is
    /// externalized, and hides nothing. A value without a placeholder is a
    /// credential somebody inlined, or a header that happens to need none, and
    /// nothing here can tell those apart: it is masked like any [`SecretKey`].
    ///
    /// The name is never masked — see [`Endpoint::header`].
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
        // The header is part of what identifies an endpoint: an operator
        // reading a startup line needs to see *which* header is being set, or a
        // provider expecting a different one looks like a network failure.
        match self.displayed_header() {
            Some(header) => write!(f, " [{header}]"),
            None => Ok(()),
        }
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Same treatment in Debug — `{:?}` is what `#[derive(Debug)]` on a
        // `Config` reaches for, and `yog-context`'s derives it.
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
