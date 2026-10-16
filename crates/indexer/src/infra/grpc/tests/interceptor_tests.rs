//! Tests for the gRPC half of the credential: the pair on a tonic request, and the `-bin` name a
//! metadata key refuses; validation is `infra::endpoint::credential`'s.

use super::*;

use yog_bootstrap::SecretKey;

const TOKEN: &str = "s3cret-token-value";

fn interceptor(name: &str) -> CredentialInterceptor {
    let credential =
        Credential::new(Some((name, SecretKey::for_tests(TOKEN)))).expect("a valid header");
    CredentialInterceptor::new(&credential).expect("valid as metadata too")
}

/// ⚠️ `required_endpoint_allowing_header` promises the header is sent: without it the process
/// authenticates as anonymous and succeeds wherever that is allowed.
#[test]
fn the_configured_header_is_put_on_the_request() {
    let mut interceptor = interceptor("x-token");

    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("the interceptor never cancels a request");

    assert_eq!(
        request
            .metadata()
            .get("x-token")
            .map(|v| v.to_str().expect("ascii")),
        Some(TOKEN),
        "the credential must reach the wire"
    );
}

/// The sensitive flag must survive the conversion into tonic's metadata, where header dumps live.
#[test]
fn the_value_is_still_marked_sensitive_as_metadata() {
    let mut interceptor = interceptor("x-token");

    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("no refusal");

    assert!(
        request
            .metadata()
            .get("x-token")
            .expect("the header is set")
            .is_sensitive()
    );
}

/// An endpoint with no credential is a no-op, not a second code path.
#[test]
fn an_endpoint_without_a_header_sends_none() {
    let credential = Credential::new(None).expect("no header is valid");
    let mut interceptor = CredentialInterceptor::new(&credential).expect("valid");

    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("no refusal");

    assert!(
        request.metadata().keys().next().is_none(),
        "no metadata set"
    );
}

/// The name is the operator's: `authorization: Bearer {key}` is as legitimate as `x-token`.
#[test]
fn any_header_name_the_operator_writes_is_used() {
    let mut interceptor = interceptor("authorization");

    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("no refusal");

    assert!(request.metadata().get("authorization").is_some());
    assert!(
        request.metadata().get("x-token").is_none(),
        "nothing here favours one provider's convention"
    );
}

/// ⚠️ A `-bin` name is a valid HTTP header name but binary metadata in gRPC: refused here, not in
/// `credential`, where the WebSocket path would send it.
#[test]
fn a_binary_metadata_name_is_refused_here_and_not_upstream() {
    let credential = Credential::new(Some(("x-token-bin", SecretKey::for_tests(TOKEN))))
        .expect("a valid *header* name — `credential` has no reason to refuse it");

    let error =
        CredentialInterceptor::new(&credential).expect_err("but not a valid ascii metadata key");

    assert!(
        matches!(
            error,
            GrpcListenerError::Credential(CredentialError::InvalidHeaderName { .. })
        ),
        "got {error:?}"
    );
    assert!(
        !format!("{error} {error:?}").contains(TOKEN),
        "a refusal about the name has no business quoting the value"
    );
}
