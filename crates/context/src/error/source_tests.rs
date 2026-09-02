use super::*;

/// Stands in for the API key. Deliberately unlike anything else in the
/// message, so `contains` cannot pass by accident.
const SECRET: &str = "SECRET-DE-TEST-a1b2c3d4";

/// The assertion this module exists for.
///
/// Port 1 on loopback is reserved and nothing listens there, so the request
/// fails to connect without a single packet leaving the machine — no server
/// to script, no network, nothing to flake. The error reqwest hands back
/// carries the URL, which is precisely the shape that wrote the Helius key
/// into 38 log lines on 2 September 2026.
#[tokio::test]
async fn a_transport_failure_carries_neither_the_url_nor_its_secret() {
    let url = format!("http://127.0.0.1:1/?api-key={SECRET}");

    let raw = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect_err("port 1 must refuse the connection");

    // The premise: without the conversion, the secret *is* in there. If this
    // ever stops holding, the test below would pass while proving nothing.
    assert!(
        raw.to_string().contains(SECRET),
        "reqwest no longer puts the URL in its error — this test's premise is \
         gone, and the conversion may no longer be what protects us: {raw}"
    );

    let converted = SourceError::from(raw);
    let rendered = converted.to_string();

    assert!(!rendered.contains(SECRET), "secret leaked: {rendered}");
    assert!(!rendered.contains("127.0.0.1"), "url leaked: {rendered}");
    assert!(matches!(converted, SourceError::Http(_)), "{rendered}");

    // Redaction must not degenerate into an empty message: whoever reads the
    // log still needs to know what failed.
    assert!(rendered.len() > "source HTTP error: ".len(), "{rendered}");
}
