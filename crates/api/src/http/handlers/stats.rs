//! `GET /api/stats`: protocol-wide aggregates, composed by `StatsService`.

use crate::http::{AppState, dto::StatsResponse, error::ApiError};
use axum::{Json, extract::State};

/// `GET /api/stats`
///
/// Returns the current protocol-wide statistics. Always a 200 with a payload —
/// empty inputs surface as `null`/`0` fields, never a 404.
#[utoipa::path(
    get,
    path = "/api/stats",
    responses((
        status = 200,
        description = "Totals across every observed pool.",
        body = StatsResponse
    ))
)]
pub(crate) async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<StatsResponse>, ApiError> {
    let agg = state.stats_service.get_stats().await?;
    Ok(Json(StatsResponse::from(agg)))
}
