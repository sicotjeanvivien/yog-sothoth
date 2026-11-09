//! Every route of the document refuses a query key it does not take, and takes every key
//! it documents, called through the real router.

use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::Value;
use tower::ServiceExt;
use yog_core::domain::ApiClient;

use super::document;
use crate::{
    http::build_router,
    testing::{app_state, pk},
};

/// A key no route takes.
const UNKNOWN: &str = "yog_unknown_key";

/// ⚠️ A route that does not refuse may open a stream that never ends: past this, it counts
/// as served.
const PATIENCE: Duration = Duration::from_secs(2);

/// Each `GET` path of the document, its path parameters filled with a valid value, and its
/// documented query parameters.
fn routes() -> Vec<(String, Vec<Value>)> {
    let doc = document();
    let pool = pk(1).to_string();
    doc["paths"]
        .as_object()
        .expect("the document has paths")
        .iter()
        // ⚠️ A `POST` is not sent here: these tests read the query a `GET` documents.
        .filter(|(_, item)| item.get("get").is_some())
        .map(|(path, item)| {
            let uri = path
                .replace("{protocol}", "meteora_damm_v2")
                .replace("{pool}", &pool)
                .replace("{mint}", &pool);
            let query = item["get"]["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| p["in"] == "query")
                .cloned()
                .collect();
            (uri, query)
        })
        .collect()
}

/// A value the schema accepts: its first enumerated value, or its minimum.
fn valid_value(schema: &Value) -> Option<String> {
    if let Some(first) = schema["enum"].as_array().and_then(|values| values.first()) {
        return first.as_str().map(str::to_string);
    }
    if let Some(minimum) = schema["minimum"].as_i64() {
        return Some(minimum.to_string());
    }
    schema["oneOf"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(valid_value)
}

/// The answer to `uri`, or `None` when it is still answering after [`PATIENCE`]. ⚠️ The live
/// stream gets a ticket: without one it answers `401` before reading its query.
async fn answer(uri: &str) -> Option<(StatusCode, String, String)> {
    let state = app_state();
    let mut request = Request::builder().uri(uri);
    if uri.starts_with("/api/live") {
        let client = ApiClient {
            id: 1,
            name: "dashboard".to_string(),
        };
        let (ticket, _) = state.ticket_registry.issue(client).expect("random bytes");
        request = request.header(header::AUTHORIZATION, format!("Bearer {}", ticket.expose()));
    }
    let request = request.body(Body::empty()).unwrap();
    let call = async {
        let response = build_router(state, vec![]).oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let content_type = parts
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let detail = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|json| json["detail"].as_str().map(str::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(&body).into_owned());
        (parts.status, content_type, detail)
    };
    tokio::time::timeout(PATIENCE, call).await.ok()
}

/// Against `deny_unknown_fields` removed from one parameter struct, or `NoQuery` from one
/// route that takes no query.
#[tokio::test]
async fn every_route_refuses_a_key_it_does_not_take() {
    let routes = routes();
    assert_eq!(routes.len(), 79, "one path per `GET` /api route");

    let mut served = Vec::new();
    for (uri, _) in &routes {
        let refused = match answer(&format!("{uri}?{UNKNOWN}=1")).await {
            Some((status, content_type, detail)) => {
                status == StatusCode::BAD_REQUEST
                    && content_type == "application/problem+json"
                    && detail.contains(&format!("unknown field `{UNKNOWN}`"))
            }
            None => false,
        };
        if !refused {
            served.push(uri.clone());
        }
    }
    assert!(
        served.is_empty(),
        "these routes answer a key they do not take as if it were absent: {served:?}"
    );
}

/// The control of the refusal above: against a documented key the route's struct does not
/// take — a field renamed without its `#[param]`.
#[tokio::test]
async fn every_route_takes_every_key_it_documents() {
    let mut refused = Vec::new();
    let mut tried = 0;
    for (uri, query) in routes() {
        for parameter in query {
            let name = parameter["name"].as_str().expect("a named parameter");
            // ⚠️ No value read from a schema makes a cursor valid: only its encoding does.
            if name == "cursor" {
                continue;
            }
            let example = parameter["example"].as_str().map(str::to_string);
            let Some(value) = valid_value(&parameter["schema"]).or(example) else {
                refused.push(format!(
                    "{uri}: no valid value read from the schema of `{name}`"
                ));
                continue;
            };
            tried += 1;
            let request = format!("{uri}?{name}={value}");
            match answer(&request).await {
                Some((_, _, detail))
                    if detail.contains("unknown field") || detail.contains("unknown variant") =>
                {
                    refused.push(format!("{request}: {detail}"));
                }
                // ⚠️ A refused credential never read the query: it proves nothing about the key.
                Some((StatusCode::UNAUTHORIZED, _, detail)) => {
                    refused.push(format!("{request}: its credential refused: {detail}"));
                }
                Some(_) => {}
                None => refused.push(format!("{request}: no answer within {PATIENCE:?}")),
            }
        }
    }
    assert!(
        tried > 100,
        "the documented keys must be tried, got {tried}"
    );
    assert!(
        refused.is_empty(),
        "these routes refuse a key their contract documents: {refused:?}"
    );
}
