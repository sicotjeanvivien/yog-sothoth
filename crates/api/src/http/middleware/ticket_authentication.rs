//! Who opens the live stream: the client of an `Authorization: Bearer` ticket, used up on the
//! way. Nothing else opens it, a key included.

use axum::{
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use yog_core::domain::ApiClient;

use crate::{
    application::{
        CredentialKind, TicketRegistry, Unauthorized, identify_ticket, metrics::ApiMetrics,
    },
    http::error::ApiError,
};

/// Attach the client of the request's ticket and count the request under it; refuse any other.
pub(crate) async fn authenticate_ticket(
    State(registry): State<TicketRegistry>,
    mut request: Request,
    next: Next,
) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str)
        .to_string();

    let client = match request.headers().get(header::AUTHORIZATION) {
        None => Err(Unauthorized::Missing),
        Some(credential) => client_of(credential, &registry),
    };
    let client = match client {
        Ok(client) => client,
        Err(reason) => {
            ApiMetrics::record_unauthorized(CredentialKind::Ticket, reason);
            return ApiError::Unauthorized(CredentialKind::Ticket).into_response();
        }
    };

    ApiMetrics::record_request(&client.name, &route);
    request.extensions_mut().insert(client);
    next.run(request).await
}

/// The client `credential`'s ticket names; a header that is not text names no scheme.
fn client_of(
    credential: &HeaderValue,
    registry: &TicketRegistry,
) -> Result<ApiClient, Unauthorized> {
    let credential = credential.to_str().map_err(|_| Unauthorized::Scheme)?;
    identify_ticket(credential, registry)
}

#[cfg(test)]
#[path = "ticket_authentication_tests.rs"]
mod tests;
