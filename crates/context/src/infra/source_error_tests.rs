use super::*;
use crate::infra::http_client;

/// Stands in for the API key, unlike anything else in the message so `contains` cannot pass by
/// accident; independent of its twin in `jupiter_price_tests.rs` on purpose.
const SECRET: &str = "SECRET-DE-TEST-a1b2c3d4";

/// Port 1 on loopback refuses without a packet leaving the machine, and reqwest's error carries
/// the URL. Through `http_client()`, the shipped client,
/// whose connect timeout bounds a host that drops port 1.
#[tokio::test]
async fn a_transport_failure_carries_neither_the_url_nor_its_secret() {
    let url = format!("http://127.0.0.1:1/?api-key={SECRET}");

    let raw = http_client()
        .get(&url)
        .send()
        .await
        .expect_err("port 1 must refuse the connection");

    // The premise: without the conversion the secret is in there, or the assertions prove nothing.
    assert!(
        raw.to_string().contains(SECRET),
        "reqwest no longer puts the URL in its error — this test's premise is \
         gone, and the conversion may no longer be what protects us: {raw}"
    );

    let converted = SourceError::from(raw);
    let rendered = converted.to_string();

    assert!(!rendered.contains(SECRET), "secret leaked: {rendered}");
    // The query string and the URL form are gone. ⚠️ Not asserting the absence of `127.0.0.1`: the
    // address is not the secret, and a diagnostic change in hyper-util would redden it.
    assert!(
        !rendered.contains("api-key="),
        "query string leaked: {rendered}"
    );
    assert!(!rendered.contains("http://"), "url leaked: {rendered}");
    assert!(matches!(converted, SourceError::Http(_)), "{rendered}");

    // ⚠️ The message must still say what failed: without its URL reqwest renders every transport
    // failure alike, so a cause is appended. Separators are counted; hyper's wording may change.
    assert!(
        rendered.matches(": ").count() >= 2,
        "the cause chain was not appended, so the message no longer says what \
         failed: {rendered}"
    );
}
