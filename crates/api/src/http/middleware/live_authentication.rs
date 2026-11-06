//! Who opens the live stream: the client of a live token, consumed by this opening. A request
//! without one is refused (401), an API key included.

use axum::{
    extract::{Request, State},
    http::{HeaderValue, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::authentication::route_of;
use crate::{
    application::{LiveScope, LiveTokens, Unauthorized, identify_live, metrics::ApiMetrics},
    http::error::ApiError,
};

/// Open the stream to the client of its live token, and count the opening under it.
pub(crate) async fn authenticate_live(
    State(tokens): State<LiveTokens>,
    request: Request,
    next: Next,
) -> Response {
    let route = route_of(&request);
    let opened = request
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or(Unauthorized::Missing)
        .and_then(|credential| client_of(credential, &asked(request.uri()), &tokens));
    match opened {
        Ok(client) => {
            ApiMetrics::record_request(&client, &route);
            next.run(request).await
        }
        Err(reason) => {
            ApiMetrics::record_unauthorized(reason);
            ApiError::LiveUnauthorized.into_response()
        }
    }
}

/// The client `credential` names for `asked`; a header that is not text names no scheme.
fn client_of(
    credential: &HeaderValue,
    asked: &LiveScope,
    tokens: &LiveTokens,
) -> Result<String, Unauthorized> {
    let credential = credential.to_str().map_err(|_| Unauthorized::Scheme)?;
    identify_live(credential, asked, tokens, chrono::Utc::now().timestamp())
}

/// The scope an opening asks for: its path, and the topics of its query, split as the handler
/// splits them. ⚠️ Not validated here: a topic the stream does not serve is the handler's 400.
fn asked(uri: &Uri) -> LiveScope {
    let topics = url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .filter(|(key, _)| key == "topics")
        .flat_map(|(_, value)| value.split(',').map(str::to_string).collect::<Vec<_>>())
        .collect();
    LiveScope {
        path: uri.path().to_string(),
        topics,
    }
}

#[cfg(test)]
#[path = "live_authentication_tests.rs"]
mod tests;
