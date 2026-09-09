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
//! Nothing here decides *whether* there is a header, and nothing here validates
//! it either — that is `infra::credential`, once, for both ingestion paths.
//! What is left in this file is the one thing only gRPC does: turning a
//! validated header into request metadata.
//!
//! Whether there is one at all is the operator's, through
//! `INGEST_STREAM_HEADER_NAME` / `_HEADER_VALUE`, and
//! `yog_bootstrap::Endpoint` is what assembles the two with the key — see its
//! module docs for the four authentication shapes measured across providers,
//! and why the header's **name** is a provider convention too. This module
//! takes what `Endpoint::header()` hands over, or nothing at all, and puts it
//! on the wire.
//!
//! # ⚠️ Calling `required_endpoint_allowing_header` is a promise, and this is one of the two places it is kept
//!
//! `yog-bootstrap` cannot check that the code holding an `Endpoint` actually
//! sends its header; the two doors are separate function names precisely
//! because only the caller knows. `bootstrap/config.rs` walks through the wide
//! door for `INGEST_STREAM` **whatever the source**, because both listeners
//! keep the promise: this module for gRPC, `Credential::ws_request` for the
//! WebSocket handshake.

use tonic::{
    Status,
    metadata::{Ascii, MetadataKey, MetadataValue},
    service::Interceptor,
};

use crate::{
    error::{CredentialError, GrpcListenerError},
    infra::Credential,
};

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
    /// Build one from the already-validated [`Credential`].
    ///
    /// # Why there is a second conversion here
    ///
    /// Because tonic will not take an `http::HeaderName` for a metadata key:
    /// `MetadataKey<Ascii>` refuses names ending in `-bin`, which are *binary*
    /// metadata and a different type. That is a gRPC rule, not a header rule,
    /// so it is checked here rather than in [`Credential`] — where it would
    /// refuse a name the WebSocket path would have accepted.
    ///
    /// ⚠️ **And the sensitive flag has to be set again.** It does not travel
    /// with the bytes: `MetadataValue::try_from` builds a fresh value and knows
    /// nothing of the `HeaderValue` they came from. Caught by
    /// `the_value_is_still_marked_sensitive_as_metadata` on 10 September 2026,
    /// when this module started taking an already-validated `Credential` — the
    /// guard held one line upstream and silently stopped here.
    pub(crate) fn new(credential: &Credential) -> Result<Self, GrpcListenerError> {
        let header = credential
            .header()
            .map(|(name, value)| -> Result<_, GrpcListenerError> {
                let key =
                    MetadataKey::<Ascii>::from_bytes(name.as_str().as_bytes()).map_err(|_| {
                        // `-bin` is the only shape a valid header name can take
                        // that a metadata key cannot, and it means the operator
                        // asked for binary metadata — which this endpoint does
                        // not carry.
                        GrpcListenerError::Credential(CredentialError::InvalidHeaderName {
                            name: name.as_str().to_string(),
                        })
                    })?;

                let mut value =
                    MetadataValue::<Ascii>::try_from(value.as_bytes()).map_err(|_| {
                        GrpcListenerError::Credential(CredentialError::InvalidHeaderValue {
                            name: name.as_str().to_string(),
                        })
                    })?;
                value.set_sensitive(true);

                Ok((key, value))
            })
            .transpose()?;

        Ok(Self { header })
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
        match &self.header {
            Some((name, _)) => write!(f, "CredentialInterceptor({}: <redacted>)", name.as_str()),
            None => f.write_str("CredentialInterceptor(no header)"),
        }
    }
}

#[cfg(test)]
#[path = "interceptor_tests.rs"]
mod tests;
