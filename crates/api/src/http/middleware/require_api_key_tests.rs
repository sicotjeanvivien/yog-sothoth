use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode, header},
};
use tower::ServiceExt;
use yog_core::domain::{ActiveKey, ApiClient, ApiKey, Ticket};

use crate::http::{AppState, build_router};
use crate::testing::{app_state, counter, key_ring, recorded};

fn key(byte: u8) -> ApiKey {
    ApiKey::from_random([byte; 4], [byte; 32])
}

fn state() -> AppState {
    AppState {
        key_ring: key_ring(vec![ActiveKey {
            hash: key(1).hash(),
            client: ApiClient {
                id: 7,
                name: "dashboard".to_string(),
            },
        }]),
        ..app_state()
    }
}

async fn issue(router: Router, authorization: Option<String>) -> (StatusCode, String) {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/tickets");
    if let Some(value) = authorization {
        request = request.header(
            header::AUTHORIZATION,
            HeaderValue::from_str(&value).unwrap(),
        );
    }
    let response = router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// Mutation: `require_api_key` not mounted, or letting an anonymous request through.
#[test]
fn a_ticket_without_a_key_is_refused_as_missing() {
    let ((status, body), metrics) =
        recorded(async { issue(build_router(state(), vec![]), None).await });

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("a valid API key is required"), "{body}");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("credential", "key"), ("reason", "missing")]
        ),
        Some(1)
    );
}

/// Mutation: `authenticate` taken off the route, so a forged key reaches `require_api_key`.
#[test]
fn a_ticket_with_a_key_the_ring_does_not_hold_is_refused_as_unknown() {
    let forged = Some(format!("Bearer {}", key(2).expose()));
    let ((status, _), metrics) =
        recorded(async { issue(build_router(state(), vec![]), forged).await });

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("credential", "key"), ("reason", "unknown")]
        ),
        Some(1)
    );
}

/// Mutation: the ticket answered in another shape, or the key's client not the one counted.
#[test]
fn a_key_is_exchanged_for_a_ticket() {
    let bearer = Some(format!("Bearer {}", key(1).expose()));
    let ((status, body), metrics) =
        recorded(async { issue(build_router(state(), vec![]), bearer).await });

    assert_eq!(status, StatusCode::OK, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("a JSON body");
    let ticket = json["ticket"].as_str().expect("a ticket");
    assert!(Ticket::parse(ticket).is_ok(), "{ticket}");
    assert!(json["expiresAt"].as_str().is_some(), "{body}");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_tickets_issued_total",
            &[("client", "dashboard")]
        ),
        Some(1)
    );
}

/// Mutation: the route issuing from another registry than the live stream reads, so every
/// ticket it hands out is refused.
#[tokio::test]
async fn the_ticket_the_route_issues_opens_the_live_stream() {
    let state = state();
    let bearer = Some(format!("Bearer {}", key(1).expose()));
    let (status, body) = issue(build_router(state.clone(), vec![]), bearer).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("a JSON body");
    let ticket = json["ticket"].as_str().expect("a ticket");

    let request = Request::builder()
        .uri("/api/live?topics=signals")
        .header(header::AUTHORIZATION, format!("Bearer {ticket}"))
        .body(Body::empty())
        .unwrap();
    let response = build_router(state, vec![]).oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
