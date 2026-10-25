//! The operator's announcements: every one, paged, and the dashboard banner's active set,
//! unpaginated.

use crate::http::{
    AppState,
    dto::{
        AnnouncementEntryResponse, AnnouncementResponse, PageResponse,
        request::{AnnouncementsParams, ListAnnouncementsRequest},
    },
    error::ApiError,
};
use axum::{
    Json,
    extract::{Query, State},
};

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

#[utoipa::path(
    get,
    path = "/api/announcements",
    params(AnnouncementsParams),
    responses((
        status = 200,
        description = "A page of every announcement, or of one status, the latest start first.",
        body = PageResponse<AnnouncementEntryResponse>
    ))
)]
pub(crate) async fn list_announcements(
    State(state): State<AppState>,
    Query(params): Query<AnnouncementsParams>,
) -> Result<Json<PageResponse<AnnouncementEntryResponse>>, ApiError> {
    let request = ListAnnouncementsRequest::parse(params)?;
    let page = state
        .announcement_service
        .list_entries(request.status, request.cursor, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}

#[cfg(test)]
#[path = "announcements_tests.rs"]
mod tests;
