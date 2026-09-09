//! The credential, put on every request — and printed by nothing.
//!
//! This is the module that justifies refusing `yellowstone-grpc-client`. That
//! crate's `InterceptorXToken` and `GeyserGrpcBuilder` both `#[derive(Debug)]`
//! over a `pub x_token: Option<AsciiMetadataValue>`, and an
//! `AsciiMetadataValue` is not marked sensitive — so any `{:?}` anywhere in the
//! process writes the token in the clear, and "the token appears in no log"
//! becomes a property of a third party's derive. Ten lines of our own keep it a
//! property of this file.
//!
//! # What carries the credential, and what does not
//!
//! Nothing here decides *whether* there is a header. That is the operator's,
//! through `INGEST_STREAM_HEADER_NAME` / `_HEADER_VALUE`, and
//! `yog_bootstrap::Endpoint` is what assembles the two with the key — see its
//! module docs for the four authentication shapes measured across providers,
//! and why the header's **name** is a provider convention too. This module
//! takes what `Endpoint::header()` hands over, or nothing at all, and puts it
//! on the wire.
//!
//! # ⚠️ Calling `required_endpoint_with_header` is a promise, and this is where it is kept
//!
//! `yog-bootstrap` cannot check that the code holding an `Endpoint` actually
//! sends its header; the two doors are separate function names precisely
//! because only the caller knows. `bootstrap/config.rs` walks through the
//! `_with_header` door for `INGEST_SOURCE=grpc`, and this is the code that
//! makes that true.

use tonic::{
    Status,
    metadata::{Ascii, MetadataKey, MetadataValue},
    service::Interceptor,
};
use yog_bootstrap::SecretKey;

use crate::error::GrpcListenerError;

/// Puts the configured metadata header on every outgoing request.
///
/// Holds no `Option` of its own beyond the header itself: an endpoint with no
/// credential — a self-hosted Yellowstone, an IP allowlist — builds one of
/// these with `None` and it becomes a no-op, rather than a second code path.
#[derive(Clone)]
pub(crate) struct CredentialInterceptor {
    /// The name is not secret and is the whole diagnostic; the value is.
    header: Option<(MetadataKey<Ascii>, MetadataValue<Ascii>)>,
}

impl CredentialInterceptor {
    /// Build one from what an `Endpoint` says its header is.
    ///
    /// # Why the name and the value are validated here, once
    ///
    /// Because the alternative is a `Status` per request for a configuration
    /// mistake that will never fix itself. A header name that is not a token,
    /// or a value carrying a byte no header may hold, is a startup failure —
    /// the shape of failure an operator can act on — and it is returned as a
    /// typed error rather than logged, so it reaches `main` through the same
    /// road as every other refused configuration.
    ///
    /// ⚠️ **Neither error carries the value.** `InvalidMetadataValue` does not
    /// quote it, and nothing here adds it: an error message about a malformed
    /// credential that prints the credential is the leak this whole module is
    /// written against. The name is quoted, since it is what tells the operator
    /// which line of the `.env` to look at.
    pub(crate) fn new(header: Option<(&str, SecretKey)>) -> Result<Self, GrpcListenerError> {
        let header = header
            .map(|(name, value)| {
                let key = MetadataKey::<Ascii>::from_bytes(name.as_bytes()).map_err(|_| {
                    GrpcListenerError::InvalidHeaderName {
                        name: name.to_string(),
                    }
                })?;

                let mut value = MetadataValue::<Ascii>::try_from(value.expose()).map_err(|_| {
                    GrpcListenerError::InvalidHeaderValue {
                        name: name.to_string(),
                    }
                })?;
                // The second guard, and the one that covers what this file does
                // not write: `hyper` and `tower-http` skip sensitive values when
                // they dump request headers, so a debug layer added later — by
                // us or by a dependency — cannot print it either.
                value.set_sensitive(true);

                Ok((key, value))
            })
            .transpose()?;

        Ok(Self { header })
    }

    /// The header's name, for the one startup line that says which is set.
    ///
    /// `None` when the endpoint carries no header at all — which an operator
    /// also needs to see, since a provider expecting one would otherwise look
    /// like a network failure.
    pub(crate) fn header_name(&self) -> Option<&str> {
        self.header.as_ref().map(|(name, _)| name.as_str())
    }
}

impl Interceptor for CredentialInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        if let Some((name, value)) = &self.header {
            request.metadata_mut().insert(name.clone(), value.clone());
        }
        Ok(request)
    }
}

/// ⚠️ Written by hand — and **measured**, because the obvious claim about it is
/// false.
///
/// The tempting sentence is "a `#[derive(Debug)]` here would print the token".
/// It would not, *today*: `MetadataValue`'s `Debug` delegates to
/// `http::HeaderValue`'s, which writes `Sensitive` instead of the value once
/// [`MetadataValue::set_sensitive`] has been called. Checked by mutation on
/// 9 September 2026 — deriving `Debug` keeps every test green, and only
/// dropping *both* this impl and the `set_sensitive` call above turns
/// `debug_never_prints_the_token` red.
///
/// So this is the second of two independent guards, not the only one, and it is
/// worth its ten lines for exactly that: the flag is one line in a builder that
/// a refactor can drop, and `yellowstone-grpc-client` is the proof that this is
/// not hypothetical — its `InterceptorXToken` derives `Debug` over a value it
/// never marks. It also does something the flag cannot: keep the header's
/// **name** legible, which is the diagnostic an operator needs when a provider
/// expects a different one.
impl std::fmt::Debug for CredentialInterceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.header_name() {
            Some(name) => write!(f, "CredentialInterceptor({name}: <redacted>)"),
            None => f.write_str("CredentialInterceptor(no header)"),
        }
    }
}

#[cfg(test)]
#[path = "interceptor_tests.rs"]
mod tests;
