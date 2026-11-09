//! `POST /api/auth/tickets` — a key exchanged for a ticket to the live stream.

use axum::{
    Extension, Json,
    extract::{Query, State},
};
use yog_core::domain::ApiClient;

use crate::http::{
    AppState,
    dto::{TicketResponse, request::NoQuery},
    error::ApiError,
};

/// A ticket for the key's client: it opens one connection to `/api/live`, within 30 s.
#[utoipa::path(
    post,
    path = "/api/auth/tickets",
    security(("apiKey" = [])),
    responses((
        status = 200,
        description = "A single-use ticket for one connection to the live stream.",
        body = TicketResponse
    ))
)]
pub(crate) async fn create_ticket(
    State(state): State<AppState>,
    Extension(client): Extension<ApiClient>,
    _query: Query<NoQuery>,
) -> Result<Json<TicketResponse>, ApiError> {
    let issued = state
        .ticket_registry
        .issue(client)
        .map_err(|error| ApiError::Internal(format!("no random bytes for a ticket: {error}")))?;
    Ok(Json(issued.into()))
}
