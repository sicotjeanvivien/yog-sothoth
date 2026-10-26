//! The operator's announcements: every one, paged, or those of one status.

use crate::http::{
    AppState,
    dto::{
        AnnouncementEntryResponse, PageResponse,
        request::{AnnouncementsParams, ListAnnouncementsRequest},
    },
    error::ApiError,
};
use axum::{
    Json,
    extract::{Query, State},
};

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
