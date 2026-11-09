use axum::{
    Extension, Router,
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::from_fn_with_state,
    routing::get,
};
use tower::ServiceExt;
use yog_core::domain::{ApiClient, ApiKey, Ticket};

use super::*;
use crate::testing::{counter, recorded};

fn dashboard() -> ApiClient {
    ApiClient {
        id: 7,
        name: "dashboard".to_string(),
    }
}

/// A route that answers the name of the client the layer attached, or `-`.
fn echo(registry: TicketRegistry) -> Router {
    Router::new()
        .route(
            "/live/{id}",
            get(|client: Option<Extension<ApiClient>>| async move {
                client.map_or_else(|| "-".to_string(), |Extension(c)| c.name)
            }),
        )
        .route_layer(from_fn_with_state(registry, authenticate_ticket))
}

async fn send(router: Router, authorization: Option<&str>) -> (StatusCode, String) {
    let mut request = Request::builder().uri("/live/1");
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
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

fn issued(registry: &TicketRegistry) -> String {
    let (ticket, _) = registry.issue(dashboard()).expect("random bytes");
    format!("Bearer {}", ticket.expose())
}

/// Mutation: an anonymous request let through, so the stream opens without a ticket.
#[test]
fn a_request_without_a_ticket_is_refused() {
    let ((status, body), metrics) = recorded(send(echo(TicketRegistry::default()), None));

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(body.contains("a valid ticket is required"), "{body}");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("credential", "ticket"), ("reason", "missing")]
        ),
        Some(1)
    );
    assert_eq!(
        counter(
            &metrics,
            "yog_api_requests_total",
            &[("client", "anonymous"), ("route", "/live/{id}")]
        ),
        None
    );
}

/// Mutation: the ticket's client counted but not attached.
#[test]
fn a_ticket_attaches_its_client_and_counts_under_it() {
    let registry = TicketRegistry::default();
    let bearer = issued(&registry);
    let ((status, body), metrics) = recorded(send(echo(registry), Some(&bearer)));

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "dashboard");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_requests_total",
            &[("client", "dashboard"), ("route", "/live/{id}")]
        ),
        Some(1)
    );
}

/// A refusal, its reason counted under `ticket`, and the ticket's message.
fn refused_for(registry: TicketRegistry, authorization: &str, reason: &str) {
    let ((status, body), metrics) = recorded(send(echo(registry), Some(authorization)));

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(body.contains("a valid ticket is required"), "{body}");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("credential", "ticket"), ("reason", reason)]
        ),
        Some(1)
    );
}

// Mutation the next five are written against: `authenticate_ticket` letting the credential through.

#[test]
fn another_scheme_is_refused() {
    refused_for(TicketRegistry::default(), "Basic x", "scheme");
}

#[test]
fn a_bearer_that_is_no_ticket_is_refused() {
    refused_for(
        TicketRegistry::default(),
        "Bearer not-a-ticket",
        "malformed",
    );
}

/// Mutation: a key read as a ticket, so a client with a key opens the stream without one.
#[test]
fn a_key_is_refused_as_no_ticket() {
    let key = ApiKey::from_random([1; 4], [1; 32]);
    refused_for(
        TicketRegistry::default(),
        &format!("Bearer {}", key.expose()),
        "malformed",
    );
}

#[test]
fn a_ticket_never_issued_is_refused() {
    let forged = Ticket::from_random([9; 32]);
    refused_for(
        TicketRegistry::default(),
        &format!("Bearer {}", forged.expose()),
        "unknown",
    );
}

/// Mutation: the ticket left in the registry by the layer, so it opens twice.
#[test]
fn a_used_ticket_is_refused() {
    let registry = TicketRegistry::default();
    let bearer = issued(&registry);
    registry
        .redeem(&Ticket::parse(&bearer["Bearer ".len()..]).unwrap())
        .expect("its first use");

    refused_for(registry, &bearer, "unknown");
}

/// Mutation: the lifetime not checked on the way in.
#[test]
fn an_expired_ticket_is_refused() {
    let ((status, body), metrics) = recorded(async {
        tokio::time::pause();
        let registry = TicketRegistry::default();
        let bearer = issued(&registry);
        tokio::time::advance(crate::application::TICKET_LIFETIME).await;
        send(echo(registry), Some(&bearer)).await
    });

    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("credential", "ticket"), ("reason", "expired")]
        ),
        Some(1)
    );
}

/// The status the full router answers `path` with, its body left unread: a stream never ends.
async fn status_of(router: Router, path: &str, authorization: &str) -> StatusCode {
    let request = Request::builder()
        .uri(path)
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_str(authorization).unwrap(),
        )
        .body(Body::empty())
        .unwrap();
    router.oneshot(request).await.unwrap().status()
}

/// Mutation: the live stream left under `authenticate`, or the ticket layer mounted on every route.
#[tokio::test]
async fn the_live_stream_takes_a_ticket_and_every_other_route_a_key() {
    let state = crate::testing::app_state();
    let router = || crate::http::build_router(state.clone(), vec![]);
    let ticket = issued(&state.ticket_registry);
    let key = format!("Bearer {}", ApiKey::from_random([1; 4], [1; 32]).expose());

    assert_eq!(
        status_of(router(), "/api/live?topics=signals", &ticket).await,
        StatusCode::OK
    );
    assert_eq!(
        status_of(router(), "/api/live?topics=signals", &ticket).await,
        StatusCode::UNAUTHORIZED,
        "a ticket opens once"
    );
    assert_eq!(
        status_of(router(), "/api/live?topics=signals", &key).await,
        StatusCode::UNAUTHORIZED,
        "a key opens no stream"
    );
    let other = issued(&state.ticket_registry);
    assert_eq!(
        status_of(router(), "/api/signals", &other).await,
        StatusCode::UNAUTHORIZED,
        "a ticket opens no other route"
    );
}
