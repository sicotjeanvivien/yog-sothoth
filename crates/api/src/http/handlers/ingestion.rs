//! `GET /api/ingestion` — the status dot of the dashboard's "Solana" panel.

use crate::http::{AppState, dto::IngestionResponse, error::ApiError};
use axum::{Json, extract::State};

/// How fresh our ingestion is, judged on the last indexed event. No event at
/// all is `stale`, not an error.
#[utoipa::path(
    get,
    path = "/api/ingestion",
    responses((
        status = 200,
        description = "How fresh ingestion is.",
        body = IngestionResponse
    ))
)]
pub(crate) async fn get_ingestion(
    State(state): State<AppState>,
) -> Result<Json<IngestionResponse>, ApiError> {
    Ok(Json(state.ingestion_service.freshness().await?.into()))
}
