//! `GET /api/announcements/active`, the dashboard's operator banner: the full active set,
//! unpaginated.

use crate::http::{AppState, dto::AnnouncementResponse, error::ApiError};
use axum::{Json, extract::State};

/// The announcements whose display window contains the request instant, most
/// severe first then most recent. An empty array is the normal case.
#[utoipa::path(
    get,
    path = "/api/announcements/active",
    responses((
        status = 200,
        description = "The announcements active now.",
        body = Vec<AnnouncementResponse>
    ))
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
