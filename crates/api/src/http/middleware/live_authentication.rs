//! Who opens the live stream: the client of a live token, consumed by this opening. A request
//! without one is refused (401), an API key included.

use axum::{
    extract::{Query, Request, State},
    http::{HeaderValue, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::authentication::route_of;
use crate::{
    application::{LiveScope, LiveTokens, Unauthorized, identify_live, metrics::ApiMetrics},
    http::{
        dto::request::{LiveParams, LiveRequest},
        error::ApiError,
    },
};

/// Open the stream to the client of its live token, and count the opening under it.
/// ⚠️ A query the handler refuses passes untouched: it answers its 400, opens nothing, and the
/// token is not spent.
pub(crate) async fn authenticate_live(
    State(tokens): State<LiveTokens>,
    request: Request,
    next: Next,
) -> Response {
    let Some(asked) = asked(request.uri()) else {
        return next.run(request).await;
    };
    let route = route_of(&request);
    let opened = request
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or(Unauthorized::Missing)
        .and_then(|credential| client_of(credential, &asked, &tokens));
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

/// The scope an opening asks for: its path, and its topics as the handler reads them; `None` for a
/// query the handler refuses.
fn asked(uri: &Uri) -> Option<LiveScope> {
    let Query(params) = Query::<LiveParams>::try_from_uri(uri).ok()?;
    let request = LiveRequest::parse(params, None).ok()?;
    Some(LiveScope {
        path: uri.path().to_string(),
        topics: request
            .topics
            .iter()
            .map(|t| t.name().to_string())
            .collect(),
    })
}

#[cfg(test)]
#[path = "live_authentication_tests.rs"]
mod tests;
