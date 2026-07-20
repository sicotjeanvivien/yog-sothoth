//! `GET /api/announcements/active` — the dashboard's operator banner.
//!
//! No parameters, no pagination: the active window holds a handful of
//! operator-curated rows at most (hard limit in the repository). The
//! client picks what to display — the API returns the full active set,
//! most severe first.

use crate::bootstrap::AppState;
use crate::http::{dto::AnnouncementResponse, error::ApiError};
use axum::{Json, extract::State};

/// `GET /api/announcements/active`
///
/// Returns the announcements whose display window contains the request
/// instant, most severe first then most recent. An empty array is the
/// normal case, not an error.
pub(crate) async fn list_active_announcements(
    State(state): State<AppState>,
) -> Result<Json<Vec<AnnouncementResponse>>, ApiError> {
    let announcements = state.announcement_service.active().await?;

    Ok(Json(
        announcements
            .into_iter()
            .map(AnnouncementResponse::from)
            .collect(),
    ))
}
