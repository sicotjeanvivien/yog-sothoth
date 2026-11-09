//! Response DTO for `POST /api/auth/tickets`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::Ticket;

/// A ticket for one connection to the live stream, sent as `Authorization: Bearer <ticket>`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TicketResponse {
    /// `yogt_` and 64 lowercase hex characters; it opens one stream, once.
    ticket: String,

    /// When it stops opening anything, if still unused.
    expires_at: DateTime<Utc>,
}

impl From<(Ticket, DateTime<Utc>)> for TicketResponse {
    fn from((ticket, expires_at): (Ticket, DateTime<Utc>)) -> Self {
        Self {
            ticket: ticket.expose().to_string(),
            expires_at,
        }
    }
}
