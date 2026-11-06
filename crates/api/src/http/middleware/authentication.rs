//! Who calls: the client of an `Authorization: Bearer` key, or nobody. A request without a key
//! passes, counted as `anonymous`; a key that opens nothing is refused (401).

use axum::{
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use yog_core::domain::ApiClient;

use crate::{
    application::{
        KeyRing, Unauthorized, identify,
        metrics::{ANONYMOUS, ApiMetrics},
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

    let client = match client_of(credential, &ring) {
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

/// The client `credential` names; a header that is not text names no scheme.
fn client_of(credential: &HeaderValue, ring: &KeyRing) -> Result<ApiClient, Unauthorized> {
    let credential = credential.to_str().map_err(|_| Unauthorized::Scheme)?;
    identify(credential, ring)
}

#[cfg(test)]
#[path = "authentication_tests.rs"]
mod tests;
