//! `GET /api/announcements/active` — the dashboard's operator banner.
//!
//! No parameters, no pagination: the active window holds a handful of
//! operator-curated rows at most (hard limit in the repository). The
//! client picks what to display — the API returns the full active set,
//! most severe first.

use crate::http::{AppState, dto::AnnouncementResponse, error::ApiError};
use axum::{Json, extract::State};

/// `GET /api/announcements/active`
///
/// Returns the announcements whose display window contains the request
/// instant, most severe first then most recent. An empty array is the
/// normal case, not an error.
#[utoipa::path(
    get,
    path = "/api/announcements/active",
    responses((status = 200, body = Vec<AnnouncementResponse>))
)]
pub(crate) async fn list_active_announcements(
    State(state): State<AppState>,
) -> Result<Json<Vec<AnnouncementResponse>>, ApiError> {
    let announcements = state.announcement_service.list_active().await?;

    Ok(Json(
        announcements
            .into_iter()
            .map(AnnouncementResponse::from)
            .collect(),
    ))
}
