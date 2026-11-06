use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use yog_core::domain::ApiKey;

use crate::http::AppState;
use crate::testing::{app_state, counter, head_on, live_bearer, recorded, send_on};

fn open(uri: &str, authorization: Option<String>) -> Request<Body> {
    let mut request = Request::builder().uri(uri);
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    request.body(Body::empty()).unwrap()
}

/// A refusal of the live stream: 401, `WWW-Authenticate: Bearer`, and its reason counted. The
/// request is built on the state it is sent to, whose registry issues its tokens.
fn refused_for(request: impl FnOnce(&AppState) -> Request<Body>, reason: &str) {
    // ⚠️ The state is built inside the runtime: its pool needs one.
    let ((status, headers, detail), metrics) = recorded(async {
        let state = app_state();
        let request = request(&state);
        send_on(state, request).await
    });

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(headers[header::WWW_AUTHENTICATE], "Bearer");
    assert_eq!(
        detail, "the live stream opens with a live token from POST /api/live-tokens",
        "the same for every reason, and never about an API key"
    );
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("reason", reason)]
        ),
        Some(1)
    );
}

/// Mutation: the live stream left open to an anonymous call, as the requests still are.
#[test]
fn the_live_stream_refuses_a_call_without_a_token() {
    refused_for(|_| open("/api/live?topics=signals", None), "missing");
}

/// Mutation: an API key accepted on the live stream, which takes a live token only.
#[test]
fn the_live_stream_refuses_an_api_key() {
    let key = ApiKey::from_random([1; 4], [1; 32]);
    refused_for(
        |_| {
            open(
                "/api/live?topics=signals",
                Some(format!("Bearer {}", key.expose())),
            )
        },
        "key_on_live",
    );
}

/// Mutation: the topics of the query left out of the scope asked, so a token opens any topic.
#[test]
fn a_token_does_not_open_a_topic_it_was_not_issued_for() {
    refused_for(
        |state| {
            open(
                "/api/live?topics=signals,pool-creations",
                Some(live_bearer(state, "/api/live?topics=signals")),
            )
        },
        "token_scope",
    );
}

/// Mutation: the opening counted as anonymous, or not counted at all.
#[test]
fn an_opening_is_counted_under_the_client_of_its_token() {
    let ((status, _), metrics) = recorded(async {
        let state = app_state();
        let request = open(
            "/api/live?topics=signals",
            Some(live_bearer(&state, "/api/live?topics=signals")),
        );
        head_on(state, request).await
    });

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        counter(
            &metrics,
            "yog_api_requests_total",
            &[("client", "tests"), ("route", "/api/live")]
        ),
        Some(1)
    );
}

/// Mutation: the live layer mounted on the requests too, so they need a token.
#[tokio::test]
async fn the_requests_stay_open_without_a_key() {
    let (status, _, detail) = send_on(app_state(), open("/api/windows", None)).await;

    assert_eq!(status, StatusCode::OK, "{detail}");
}

/// Mutation: the topics split apart from the handler's parse — a query it refuses spends the
/// token, or reads as a 401 rather than its 400.
#[tokio::test]
async fn a_query_the_handler_refuses_is_its_400_and_spends_no_token() {
    let state = app_state();
    let bearer = live_bearer(&state, "/api/live?topics=signals");

    let (refused, _, detail) = send_on(
        state.clone(),
        open(
            "/api/live?topics=signals&topics=signals",
            Some(bearer.clone()),
        ),
    )
    .await;
    let (opened, _) = head_on(state, open("/api/live?topics=signals", Some(bearer))).await;

    assert_eq!(refused, StatusCode::BAD_REQUEST, "{detail}");
    assert_eq!(opened, StatusCode::OK, "the token was not spent");
}
