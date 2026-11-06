use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use yog_core::domain::{ActiveKey, ApiClient, ApiKey};

use crate::http::AppState;
use crate::testing::{app_state, counter, head_on, key_ring, recorded, send_on};

fn key() -> ApiKey {
    ApiKey::from_random([1; 4], [1; 32])
}

/// [`app_state`] whose ring holds [`key`], for the client `dashboard`.
fn keyed_state() -> AppState {
    let mut state = app_state();
    state.key_ring = key_ring(vec![ActiveKey {
        hash: key().hash(),
        client: ApiClient {
            id: 7,
            name: "dashboard".to_string(),
        },
    }]);
    state
}

fn ask(authorization: Option<String>, body: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/live-tokens")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    request.body(Body::from(body.to_string())).unwrap()
}

fn bearer_key() -> Option<String> {
    Some(format!("Bearer {}", key().expose()))
}

const SIGNALS: &str = r#"{"path":"/api/live","topics":["signals"]}"#;

/// Mutation: the handler issuing a token to an anonymous call.
#[test]
fn a_token_is_refused_without_a_key_and_counted_as_missing() {
    // ⚠️ The state is built inside the runtime: its pool needs one.
    let ((status, headers, _), metrics) =
        recorded(async { send_on(keyed_state(), ask(None, SIGNALS)).await });

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(headers[header::WWW_AUTHENTICATE], "Bearer");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("reason", "missing")]
        ),
        Some(1)
    );
}

#[tokio::test]
async fn a_path_outside_the_live_stream_is_a_400() {
    let (status, _, detail) = send_on(
        keyed_state(),
        ask(
            bearer_key(),
            r#"{"path":"/api/swaps","topics":["signals"]}"#,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{detail}");
    assert_eq!(detail, "`path` is no path of the live stream: /api/swaps");
}

/// Mutation: a token issued but not one the live stream opens, or one that opens twice.
#[tokio::test]
async fn a_token_issued_opens_its_stream_once() {
    let state = keyed_state();
    let (status, _, body) = send_on(state.clone(), ask(bearer_key(), SIGNALS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let issued: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(issued["tokenType"], "Bearer");
    assert_eq!(issued["expiresIn"], 30);

    let open = || {
        Request::builder()
            .uri("/api/live?topics=signals")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", issued["accessToken"].as_str().unwrap()),
            )
            .body(Body::empty())
            .unwrap()
    };
    let (first, headers) = head_on(state.clone(), open()).await;
    let (second, _) = head_on(state, open()).await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(second, StatusCode::UNAUTHORIZED);
}
