//! Tests for the credential carrier.
//!
//! These are worth more than most of this path's tests: they do not depend on
//! what a provider sends. A header either goes out or it does not, and a
//! `{:?}` either prints a secret or it does not — both are decided here, with
//! no wire involved.

use super::*;

/// The token every test looks for afterwards. Distinctive on purpose: a
/// substring search for it must not match anything a redaction leaves behind.
const TOKEN: &str = "s3cret-token-value";

fn interceptor(name: &str) -> CredentialInterceptor {
    CredentialInterceptor::new(Some((name, SecretKey::for_tests(TOKEN))))
        .expect("a valid header name and value")
}

// ── the header actually goes out ────────────────────────────────────

/// ⚠️ **The half `yog-bootstrap` cannot check.** `required_endpoint_with_header`
/// is a promise that the consumer sends the header, and this is the test that
/// the promise is kept. Without it the process would authenticate as anonymous
/// and succeed against any endpoint that allows it — silence, and a stream that
/// looks fine until it is refused.
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

/// An endpoint with no credential — a self-hosted Yellowstone ships
/// `"x_token": null` — is a no-op, not a second code path.
#[test]
fn an_endpoint_without_a_header_sends_none() {
    let mut interceptor = CredentialInterceptor::new(None).expect("no header is valid");

    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("no refusal");

    assert!(
        request.metadata().keys().next().is_none(),
        "no metadata set"
    );
    assert_eq!(interceptor.header_name(), None);
}

// ── the token is printed by nothing ─────────────────────────────────

/// ⚠️ **The acceptance criterion of the ticket, in the one form a test can carry
/// it.** `yellowstone-grpc-client` fails this exact assertion — its
/// `InterceptorXToken` derives `Debug` over an `AsciiMetadataValue` it never
/// marks sensitive — and that is the measurable reason it is not a dependency.
///
/// ⚠️ What turns this red is **both** guards going: measured by mutation on
/// 9 September 2026, replacing the hand-written `Debug` by a derive keeps it
/// green on its own, because `set_sensitive` makes `HeaderValue`'s own `Debug`
/// print `Sensitive`. Written down because the natural reading of this test —
/// "it pins the manual impl" — is wrong, and a test believed to guard something
/// it does not is worse than no test.
#[test]
fn debug_never_prints_the_token() {
    let interceptor = interceptor("x-token");

    let printed = format!("{interceptor:?}");

    assert!(
        !printed.contains(TOKEN),
        "the token must not survive a `{{:?}}`, and it did: {printed}"
    );
    assert!(
        printed.contains("x-token"),
        "the header *name* must stay legible — it is the diagnostic that tells \
         an operator their provider expects a different one"
    );
}

/// The second guard, and the one that covers code this repository does not
/// write: `hyper` and `tower-http` honour the sensitive flag when they dump
/// request headers, so a debug layer added later cannot print it either.
#[test]
fn the_header_value_is_marked_sensitive() {
    let mut interceptor = interceptor("x-token");

    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("no refusal");

    assert!(
        request
            .metadata()
            .get("x-token")
            .expect("the header is set")
            .is_sensitive(),
        "an unmarked value is one a third-party header dump prints in full"
    );
}

// ── refusals name the line to fix, never the secret ─────────────────

/// A malformed name is a startup failure, not a `Status` per request: the
/// configuration will not fix itself between two calls.
#[test]
fn a_header_name_that_is_not_a_token_is_refused_at_construction() {
    let error = CredentialInterceptor::new(Some(("x token", SecretKey::for_tests(TOKEN))))
        .expect_err("a space is not allowed in a header name");

    assert!(
        matches!(error, GrpcListenerError::InvalidHeaderName { .. }),
        "got {error:?}"
    );
    assert!(
        !error.to_string().contains(TOKEN),
        "a refusal about the name has no business quoting the value"
    );
}

/// ⚠️ And the refusal about the **value** must not quote the value either —
/// which is the one an error message would most naturally do, since that is
/// what is wrong with it.
#[test]
fn a_refused_header_value_is_never_quoted_back() {
    // A newline cannot appear in a header value; it is also what a credential
    // pasted with its line ending looks like.
    let broken = format!("{TOKEN}\n");
    let error = CredentialInterceptor::new(Some(("x-token", SecretKey::for_tests(&broken))))
        .expect_err("a newline is not allowed in a header value");

    assert!(
        matches!(error, GrpcListenerError::InvalidHeaderValue { .. }),
        "got {error:?}"
    );
    let printed = format!("{error} {error:?}");
    assert!(
        !printed.contains(TOKEN),
        "neither Display nor Debug may carry the credential: {printed}"
    );
    assert!(
        printed.contains("x-token"),
        "the header name is what the operator needs to find the line"
    );
}
