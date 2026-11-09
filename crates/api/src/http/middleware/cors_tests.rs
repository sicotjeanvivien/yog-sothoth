use axum::{
    Router,
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, header},
    routing::post,
};
use tower::ServiceExt;

use super::cors_layer;

const DASHBOARD: &str = "https://yog-scope.xyz";

/// The answer to the preflight a browser sends before a `POST` that carries `Authorization`.
async fn preflight(origin: &str) -> HeaderMap {
    let router = Router::new()
        .route("/api/auth/tickets", post(|| async { "" }))
        .layer(cors_layer(vec![HeaderValue::from_static(DASHBOARD)]));
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/auth/tickets")
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
        .body(Body::empty())
        .unwrap();
    router.oneshot(request).await.unwrap().headers().clone()
}

/// Mutation: `POST` or `Authorization` left out, so a browser cannot ask for a ticket.
#[tokio::test]
async fn an_allowed_origin_may_post_with_authorization() {
    let response_headers = preflight(DASHBOARD).await;

    assert_eq!(
        response_headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static(DASHBOARD))
    );
    let methods = response_headers[header::ACCESS_CONTROL_ALLOW_METHODS]
        .to_str()
        .unwrap()
        .to_ascii_uppercase();
    assert!(methods.contains("POST"), "{methods}");
    let headers = response_headers[header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .unwrap()
        .to_ascii_lowercase();
    assert!(headers.contains("authorization"), "{headers}");
}

/// Mutation: the origins opened to anyone (`Any`), now that a browser may send a credential.
#[tokio::test]
async fn another_origin_is_not_allowed() {
    let allowed = |origin| async move {
        preflight(origin)
            .await
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .cloned()
    };

    assert_eq!(allowed("https://evil.example").await, None);
    assert_eq!(
        allowed(DASHBOARD).await,
        Some(HeaderValue::from_static(DASHBOARD))
    );
}
