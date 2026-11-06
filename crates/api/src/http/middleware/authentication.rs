//! Who calls: the client of an `Authorization: Bearer` key, or nobody. A request without a key
//! passes, counted as `anonymous`; a key that opens nothing is refused (401).
//! ⚠️ A route layer: it runs after routing, so the route template is known and a probe is not one.

use axum::{
    extract::{MatchedPath, Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use yog_core::domain::ApiKey;

use crate::{
    application::{
        KeyRing,
        metrics::{ANONYMOUS, ApiMetrics, Unauthorized},
    },
    http::error::ApiError,
};

/// Attach the request's client, if it names one, and count the request under it.
pub(crate) async fn authenticate(
    State(ring): State<KeyRing>,
    mut request: Request,
    next: Next,
) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str)
        .to_string();

    let Some(credential) = request.headers().get(header::AUTHORIZATION) else {
        ApiMetrics::record_request(ANONYMOUS, &route);
        return next.run(request).await;
    };

    let client = match credential
        .to_str()
        .ok()
        .and_then(bearer)
        .ok_or(Unauthorized::Scheme)
        .and_then(|token| ApiKey::parse(token).map_err(|_| Unauthorized::Malformed))
        .and_then(|key| ring.client_of(&key).ok_or(Unauthorized::Unknown))
    {
        Ok(client) => client,
        Err(reason) => {
            ApiMetrics::record_unauthorized(reason);
            return ApiError::Unauthorized.into_response();
        }
    };

    ApiMetrics::record_request(&client.name, &route);
    request.extensions_mut().insert(client);
    next.run(request).await
}

/// The token of a `Bearer` credential. ⚠️ The scheme is case-insensitive (RFC 9110 §11.1).
fn bearer(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

#[cfg(test)]
#[path = "authentication_tests.rs"]
mod tests;
