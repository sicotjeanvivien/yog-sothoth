//! `GET /api/windows` — which windows the service opens, without reading a measure over them.

use crate::http::{
    AppState,
    dto::{WindowsResponse, request::NoQuery},
    error::ApiError,
};
use axum::{
    Json,
    extract::{Query, State},
};

#[utoipa::path(
    get,
    path = "/api/windows",
    responses((
        status = 200,
        description = "Every window, its bounds, whether it is open, when it opens, and whether its \
                       figures run late.",
        body = WindowsResponse
    ))
)]
pub(crate) async fn get_windows(
    State(state): State<AppState>,
    _query: Query<NoQuery>,
) -> Result<Json<WindowsResponse>, ApiError> {
    Ok(Json(WindowsResponse::new(
        state.windows_service.list().await?,
    )))
}
