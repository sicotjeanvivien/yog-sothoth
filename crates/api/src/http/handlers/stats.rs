//! `GET /api/stats` — protocol-wide aggregate statistics.
//!
//! Client-agnostic: powers the dashboard Overview, but ships raw aggregates
//! (TVL + coverage counters, 24h volume/fees, pool counts) usable by any
//! client. The handler only orchestrates the read and shapes the response;
//! the composition (analytics + counts) lives in `StatsService`.

use crate::bootstrap::AppState;
use crate::http::{dto::StatsResponse, error::ApiError};
use axum::{Json, extract::State};

/// `GET /api/stats`
///
/// Returns the current protocol-wide statistics. Always a 200 with a payload —
/// empty inputs surface as `null`/`0` fields, never a 404.
pub(crate) async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<StatsResponse>, ApiError> {
    let agg = state.stats_service.get_stats().await?;
    Ok(Json(StatsResponse::from(agg)))
}
