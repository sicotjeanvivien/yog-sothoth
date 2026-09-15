//! The credential an endpoint carries in a header, validated once.
//!
//! # Why this is not per-transport
//!
//! Because the operator's configuration is not per-transport. Whether there is
//! a header is said by `<FUNCTION>_HEADER_NAME` / `<FUNCTION>_HEADER_VALUE` and
//! by nothing else — not by `INGEST_SOURCE`, which names an acquisition model
//! and has no business deciding what a provider wants for authentication.
//!
//! ⚠️ **This module exists because an earlier shape had it the other way**, and
//! the objection that removed it is worth keeping: `bootstrap/config.rs` chose
//! the reading door on `INGEST_SOURCE`, so a header configured for the
//! WebSocket path was refused at startup. That put a *transport* in charge of a
//! *credential* question — the very inversion
//! `04 - release/une-variable-nomme-un-transport.md` was written against — and
//! it rested on a claim that turned out to be false: that the WebSocket client
//! could not send a header. It can. `PubsubClient::new` takes an
//! `IntoClientRequest`, so what was missing was not a capability but ten lines,
//! and those are [`Credential::ws_request`] below. Raised in review of PR #138,
//! 10 September 2026.
//!
//! So both paths now read `INGEST_STREAM` through the same door, and both keep
//! the promise that door makes.
//!
//! # What "validated once" buys
//!
//! A header name that is not a token, or a value carrying a byte no header may
//! hold, is a **configuration** failure: it will not fix itself between two
//! connection attempts. Validating at construction turns it into a startup
//! error that names the variable, instead of a per-attempt failure that burns a
//! retry budget and reads like a network fault.
//!
//! # What is done about printing it
//!
//! Two guards, and they do not overlap the way one would assume — see
//! [`Credential`]'s `Debug`.

use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    handshake::client::Request as WsRequest,
    http::{HeaderName, HeaderValue},
};
use yog_bootstrap::{SecretKey, SecretUrl};

use crate::error::CredentialError;

/// An endpoint's header, ready to be sent.
///
/// `None` inside is a first-class case, not a degraded one: a self-hosted
/// Yellowstone takes no credential, a provider may want it inside the URL, and
/// an IP allowlist wants nothing at all. Three of the four authentication
/// shapes measured across providers on 8 September 2026 send no header, which
/// is why the absence gets a no-op rather than a second code path.
#[derive(Clone)]
pub(crate) struct Credential {
    header: Option<(HeaderName, HeaderValue)>,
}

impl Credential {
    /// Validate what [`yog_bootstrap::Endpoint::header`] handed over.
    ///
    /// ⚠️ **Neither error carries the value.** An error message about a
    /// malformed credential that prints the credential is the leak this module
    /// is written against. The *name* is quoted, since it is what tells the
    /// operator which line of the `.env` to look at.
    pub(crate) fn new(header: Option<(&str, SecretKey)>) -> Result<Self, CredentialError> {
        let header = header
            .map(|(name, value)| {
                let name =
                    HeaderName::try_from(name).map_err(|_| CredentialError::InvalidHeaderName {
                        name: name.to_string(),
                    })?;

                let mut value = HeaderValue::from_str(value.expose()).map_err(|_| {
                    CredentialError::InvalidHeaderValue {
                        name: name.as_str().to_string(),
                    }
                })?;
                // The guard that covers what this crate does not write: `hyper`
                // and `tower-http` skip sensitive values when they dump request
                // headers, so a debug layer added later — by us or by a
                // dependency — cannot print it either.
                value.set_sensitive(true);

                Ok((name, value))
            })
            .transpose()?;

        Ok(Self { header })
    }

    /// The validated pair, for whoever puts it on the wire.
    pub(crate) fn header(&self) -> Option<(&HeaderName, &HeaderValue)> {
        self.header.as_ref().map(|(name, value)| (name, value))
    }

    /// The header's name, for the one startup line that says which is set.
    ///
    /// `None` when the endpoint carries no header — which an operator also
    /// needs to see, since a provider expecting one would otherwise look like a
    /// network failure.
    pub(crate) fn name(&self) -> Option<&str> {
        self.header.as_ref().map(|(name, _)| name.as_str())
    }

    /// The WebSocket handshake request for `url`, credential included.
    ///
    /// # ⚠️ Why the request is built from the URL rather than assembled here
    ///
    /// Because a handshake request is not just a URL: `Host`, `Connection`,
    /// `Upgrade`, `Sec-WebSocket-Version` and a freshly generated
    /// `Sec-WebSocket-Key` must all be present, and `tungstenite` refuses one
    /// that is missing any of them. `into_client_request` is what produces
    /// them; this only adds a header to what it made.
    ///
    /// The returned `http::Request<()>` is what `PubsubClient::new` accepts —
    /// the same type it takes today through the `&str` impl, one step earlier.
    ///
    /// # Errors
    ///
    /// A URL `tungstenite` will not turn into a request. The message is
    /// **scrubbed**: it quotes the URL it was handed, and that URL carries the
    /// key whenever the operator put it there rather than in a header.
    pub(crate) fn ws_request(&self, url: &SecretUrl) -> Result<WsRequest, String> {
        let mut request = url
            .expose()
            .into_client_request()
            .map_err(|e| url.scrub(&format!("websocket request: {e}")))?;

        if let Some((name, value)) = self.header() {
            request.headers_mut().insert(name, value.clone());
        }

        Ok(request)
    }
}

/// ⚠️ Written by hand — and **measured**, because the obvious claim about it is
/// false.
///
/// The tempting sentence is "a `#[derive(Debug)]` here would print the token".
/// It would not, *today*: `HeaderValue`'s `Debug` writes `Sensitive` instead of
/// the value once `set_sensitive` has been called. Checked by mutation on
/// 9 September 2026 — deriving `Debug` keeps every test green, and only
/// dropping *both* this impl and the `set_sensitive` call turns
/// `debug_never_prints_the_token` red.
///
/// So this is the second of two independent guards, not the only one, and it is
/// worth its ten lines for exactly that: the flag is one line in a constructor
/// that a refactor can drop, and `yellowstone-grpc-client` is the proof that
/// this is not hypothetical — its `InterceptorXToken` derives `Debug` over a
/// value it never marks. It also does something the flag cannot: keep the
/// header's **name** legible, which is the diagnostic an operator needs when a
/// provider expects a different one.
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.name() {
            Some(name) => write!(f, "Credential({name}: <redacted>)"),
            None => f.write_str("Credential(no header)"),
        }
    }
}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
