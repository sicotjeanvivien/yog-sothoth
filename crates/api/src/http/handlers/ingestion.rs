//! `GET /api/ingestion` — the status dot of the dashboard's "Solana" panel.

use crate::http::{
    AppState,
    dto::{IngestionResponse, request::NoQuery},
    error::ApiError,
};
use axum::{
    Json,
    extract::{Query, State},
};

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
    _query: Query<NoQuery>,
) -> Result<Json<IngestionResponse>, ApiError> {
    Ok(Json(state.ingestion_service.freshness().await?.into()))
}

#[cfg(test)]
#[path = "ingestion_tests.rs"]
mod tests;
