use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, header};
use tower::ServiceExt;

use crate::http::build_router;
use crate::testing::app_state;

const DASHBOARD: &str = "http://dashboard.test";

/// The answer to a preflight of `POST /api/live-tokens` with `Authorization`, from `origin`.
async fn preflight(origin: &str) -> axum::http::HeaderMap {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/live-tokens")
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            "authorization,content-type",
        )
        .body(Body::empty())
        .unwrap();
    build_router(app_state(), vec![HeaderValue::from_static(DASHBOARD)])
        .oneshot(request)
        .await
        .unwrap()
        .headers()
        .clone()
}

/// Mutation: `POST` or `Authorization` left out — the explorer cannot send a visitor's key,
/// nor exchange it for a live token.
#[tokio::test]
async fn the_dashboard_may_post_with_an_authorization() {
    let headers = preflight(DASHBOARD).await;

    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], DASHBOARD);
    let methods = headers[header::ACCESS_CONTROL_ALLOW_METHODS]
        .to_str()
        .unwrap();
    assert!(methods.contains("POST"), "{methods}");
    let allowed = headers[header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .unwrap();
    assert!(allowed.contains("authorization"), "{allowed}");
}

/// Mutation: every origin allowed once `Authorization` is.
#[tokio::test]
async fn another_origin_is_not_allowed() {
    let headers = preflight("http://elsewhere.test").await;

    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}
