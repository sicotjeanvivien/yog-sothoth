//! The credential an endpoint carries in a header, validated once at start-up, for both paths.
//! ⚠️ Whether there is a header is said by `<FUNCTION>_HEADER_NAME`/`_VALUE` alone, never by
//! `INGEST_SOURCE`: a transport has no say in what a provider wants for authentication.

use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    handshake::client::Request as WsRequest,
    http::{HeaderName, HeaderValue},
};
use yog_bootstrap::{SecretKey, SecretUrl};

use crate::error::CredentialError;

/// An endpoint's header, ready to be sent; `None` (no credential, or one inside the URL) is a
/// no-op, not a second code path.
#[derive(Clone)]
pub(crate) struct Credential {
    header: Option<(HeaderName, HeaderValue)>,
}

impl Credential {
    /// Validate what [`yog_bootstrap::Endpoint::header`] handed over.
    /// ⚠️ Neither error carries the value; the name is quoted, it tells the operator the `.env` line.
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
                // Covers what this crate does not write: `hyper` and `tower-http` skip sensitive
                // values when they dump request headers.
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

    /// The header's name for the start-up line, `None` when there is none: an operator needs both.
    pub(crate) fn name(&self) -> Option<&str> {
        self.header.as_ref().map(|(name, _)| name.as_str())
    }

    /// The WebSocket handshake request for `url`, credential included; `into_client_request` adds
    /// the `Sec-WebSocket-*` headers `tungstenite` requires.
    /// ⚠️ The error is scrubbed: it quotes the URL, which carries the key when no header does.
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

/// ⚠️ Written by hand, a second guard beside `set_sensitive`: a derive would rest on that one
/// constructor line alone. It also keeps the header's name legible.
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
