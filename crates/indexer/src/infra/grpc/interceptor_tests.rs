//! Tests for the gRPC half of the credential.
//!
//! Deliberately short: validating the header, refusing a malformed one without
//! quoting it, and keeping it out of `{:?}` are `infra::credential`'s business
//! and are tested there, once, for both paths. What is left here is what only
//! this module does — putting the pair on a tonic request, and the one shape a
//! metadata key refuses that a header name allows.

use super::*;

use yog_bootstrap::SecretKey;

const TOKEN: &str = "s3cret-token-value";

fn interceptor(name: &str) -> CredentialInterceptor {
    let credential =
        Credential::new(Some((name, SecretKey::for_tests(TOKEN)))).expect("a valid header");
    CredentialInterceptor::new(&credential).expect("valid as metadata too")
}

/// ⚠️ **The half `yog-bootstrap` cannot check.**
/// `required_endpoint_allowing_header` is a promise that the consumer sends the
/// header, and this is the test that the promise is kept on this path. Without
/// it the process authenticates as anonymous and succeeds against any endpoint
/// that allows it — silence, and a stream that looks fine until it is refused.
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

/// The sensitive flag has to survive the conversion into tonic's metadata, or
/// the guard stops at the crate boundary — which is exactly where the header
/// dumps that would print it live.
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

/// The name is the operator's, not ours — `authorization: Bearer {key}` is as
/// legitimate a shape as `x-token`, and the code knows neither provider.
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

/// ⚠️ The one refusal that belongs here rather than in `credential`: a name
/// ending in `-bin` is a perfectly valid HTTP header name — the WebSocket path
/// would send it — but names *binary* metadata in gRPC, a different type this
/// endpoint does not carry. A gRPC rule, checked where gRPC is.
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
