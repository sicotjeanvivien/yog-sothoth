//! The credential, put on every request and printed by nothing: our own interceptor,
//! since `yellowstone-grpc-client` derives `Debug` over a token not marked sensitive.
//! ⚠️ `INGEST_STREAM` is read through `required_endpoint_allowing_header`: this module
//! keeps that promise for gRPC, `Credential::ws_request` for the WebSocket.

use tonic::{
    Status,
    metadata::{Ascii, MetadataKey, MetadataValue},
    service::Interceptor,
};

use crate::{
    error::{CredentialError, GrpcListenerError},
    infra::Credential,
};

/// Puts the configured metadata header on every outgoing request; a no-op when there is none.
#[derive(Clone)]
pub(crate) struct CredentialInterceptor {
    /// The name is not secret and is the whole diagnostic; the value is.
    header: Option<(MetadataKey<Ascii>, MetadataValue<Ascii>)>,
}

impl CredentialInterceptor {
    /// Build one from the already-validated [`Credential`]; a `-bin` name, binary metadata, is
    /// refused here and not there, since only gRPC forbids it.
    pub(crate) fn new(credential: &Credential) -> Result<Self, GrpcListenerError> {
        let header = credential
            .header()
            .map(|(name, value)| -> Result<_, GrpcListenerError> {
                let key =
                    MetadataKey::<Ascii>::from_bytes(name.as_str().as_bytes()).map_err(|_| {
                        // `-bin` is the only valid header name a metadata key refuses.
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
                // ⚠️ Again: the flag does not travel from the `HeaderValue` the bytes came from.
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

/// ⚠️ Written by hand, a second guard beside `set_sensitive`: a derive would rest on that one
/// builder line alone. It also keeps the header's name legible.
impl std::fmt::Debug for CredentialInterceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.header {
            Some((name, _)) => write!(f, "CredentialInterceptor({}: <redacted>)", name.as_str()),
            None => f.write_str("CredentialInterceptor(no header)"),
        }
    }
}

#[cfg(test)]
#[path = "tests/interceptor_tests.rs"]
mod tests;
