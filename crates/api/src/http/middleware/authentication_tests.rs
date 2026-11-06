use std::future::Future;

use axum::{
    Extension, Router,
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::from_fn_with_state,
    routing::get,
};
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use tower::ServiceExt;
use yog_core::domain::{ActiveKey, ApiClient, ApiKey};

use super::*;

fn key(byte: u8) -> ApiKey {
    ApiKey::from_random([byte; 4], [byte; 32])
}

fn ring() -> KeyRing {
    crate::testing::key_ring(vec![ActiveKey {
        hash: key(1).hash(),
        client: ApiClient {
            id: 7,
            name: "dashboard".to_string(),
        },
    }])
}

/// A route that answers the name of the client the layer attached, or `-`.
fn echo() -> Router {
    Router::new()
        .route(
            "/echo/{id}",
            get(|client: Option<Extension<ApiClient>>| async move {
                client.map_or_else(|| "-".to_string(), |Extension(c)| c.name)
            }),
        )
        .route_layer(from_fn_with_state(ring(), authenticate))
}

struct Answer {
    status: StatusCode,
    www_authenticate: Option<HeaderValue>,
    body: String,
}

async fn send(router: Router, path: &str, authorization: Option<&str>) -> Answer {
    let mut request = Request::builder().uri(path);
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    let response = router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let www_authenticate = response.headers().get(header::WWW_AUTHENTICATE).cloned();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Answer {
        status,
        www_authenticate,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

/// Run `f` on a current-thread runtime under a recorder of its own, and hand back its snapshot.
fn recorded<T>(f: impl Future<Output = T>) -> (T, Snapshotter) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let out = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    });
    (out, snapshotter)
}

/// The value of counter `name` whose labels are exactly `labels`.
fn counter(snapshotter: &Snapshotter, name: &str, labels: &[(&str, &str)]) -> Option<u64> {
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| {
            let mut own: Vec<_> = key
                .key()
                .labels()
                .map(|l| (l.key().to_string(), l.value().to_string()))
                .collect();
            own.sort();
            let mut wanted: Vec<_> = labels
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            wanted.sort();
            key.key().name() == name && own == wanted
        })
        .and_then(|(_, _, _, value)| match value {
            DebugValue::Counter(n) => Some(n),
            _ => None,
        })
}

/// Mutation: a request without a key refused, or counted under its path rather than its route.
#[test]
fn a_request_without_a_key_passes_counted_as_anonymous_under_its_route() {
    let (answer, metrics) = recorded(send(echo(), "/echo/42", None));

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, "-", "no client attached");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_requests_total",
            &[("client", "anonymous"), ("route", "/echo/{id}")]
        ),
        Some(1)
    );
}

/// Mutation: the client counted but not attached, or attached under another key's.
#[test]
fn a_known_key_attaches_its_client_and_counts_under_it() {
    let bearer = format!("Bearer {}", key(1).expose());
    let (answer, metrics) = recorded(send(echo(), "/echo/42", Some(&bearer)));

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, "dashboard");
    assert_eq!(
        counter(
            &metrics,
            "yog_api_requests_total",
            &[("client", "dashboard"), ("route", "/echo/{id}")]
        ),
        Some(1)
    );
}

/// A refusal, its reason counted and nothing else said: 401, `WWW-Authenticate: Bearer`, one text.
fn refused_for(authorization: &str, reason: &str) -> String {
    let (answer, metrics) = recorded(send(echo(), "/echo/42", Some(authorization)));

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert_eq!(
        answer
            .www_authenticate
            .as_ref()
            .map(|v| v.to_str().unwrap()),
        Some("Bearer")
    );
    assert_eq!(
        counter(
            &metrics,
            "yog_api_unauthorized_total",
            &[("reason", reason)]
        ),
        Some(1)
    );
    assert_eq!(
        counter(
            &metrics,
            "yog_api_requests_total",
            &[("client", "anonymous"), ("route", "/echo/{id}")]
        ),
        None,
        "a refused request is not a request that reached its route"
    );
    answer.body
}

// Mutation the next three are written against: `authenticate` letting the credential through.

#[test]
fn another_scheme_is_refused() {
    refused_for(&format!("Basic {}", key(1).expose()), "scheme");
}

#[test]
fn a_bearer_that_is_no_api_key_is_refused() {
    refused_for("Bearer not-a-key", "malformed");
}

#[test]
fn a_key_the_ring_does_not_hold_is_refused() {
    refused_for(&format!("Bearer {}", key(2).expose()), "unknown");
}

/// Mutation: a refusal that says why, telling a revoked key from a forged one.
#[test]
fn every_refusal_reads_the_same() {
    let scheme = refused_for("Basic x", "scheme");
    let malformed = refused_for("Bearer x", "malformed");
    let unknown = refused_for(&format!("Bearer {}", key(2).expose()), "unknown");

    assert_eq!(scheme, malformed);
    assert_eq!(malformed, unknown);
}

/// Mutation: the layer mounted on the whole router, so a probe needs a key, or off `/api`.
#[tokio::test]
async fn the_probes_stay_outside_and_every_api_route_is_inside() {
    let forged = format!("Bearer {}", key(2).expose());

    let probe = send(
        crate::http::build_router(crate::testing::app_state(), vec![]),
        "/healthz",
        Some(&forged),
    )
    .await;
    let api = send(
        crate::http::build_router(crate::testing::app_state(), vec![]),
        "/api/signals",
        Some(&forged),
    )
    .await;

    assert_eq!(probe.status, StatusCode::OK);
    assert_eq!(api.status, StatusCode::UNAUTHORIZED);
}
