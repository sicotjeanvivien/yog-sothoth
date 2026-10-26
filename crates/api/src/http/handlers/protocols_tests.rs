use axum::http::StatusCode;

use crate::testing::get;

/// Mutation: the cursor decoded as any string, or the limit unchecked — a page after a protocol
/// this build does not know, or of 0 or 201 entries, answered.
#[tokio::test]
async fn a_catalogue_cursor_or_limit_outside_the_contract_is_refused() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let unknown = URL_SAFE_NO_PAD.encode(br#"{"protocol":"meteora"}"#);

    for uri in [
        format!("/api/protocols?cursor={unknown}"),
        "/api/protocols?cursor=%21".to_owned(),
        "/api/protocols?limit=0".to_owned(),
        "/api/protocols?limit=201".to_owned(),
    ] {
        let (status, content_type, body) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}
