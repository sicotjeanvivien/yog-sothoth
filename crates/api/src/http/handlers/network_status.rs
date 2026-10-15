//! `GET /api/network/status` — the dashboard "Solana Live" panel.
//!
//! Assembles two sources:
//!   1. the `network_status` singleton (slot + RPC latency), written
//!      by the indexer's reporter;
//!   2. ingestion freshness, derived from the most recent indexed
//!      event.
//!
//! The handler orchestrates the two reads; the freshness *rule*
//! (thresholds) lives in `core` (`FreshnessStatus`), keeping the
//! handler free of business logic.

use crate::http::{AppState, dto::NetworkStatusResponse, error::ApiError};
use axum::{Json, extract::State};

/// Where the chain is (slot, probe latency) and, apart, whether ingestion keeps
/// up. The `network_status` row is seeded by the baseline migration: its absence
/// is a 500, not a 404.
#[utoipa::path(
    get,
    path = "/api/network/status",
    responses((
        status = 200,
        description = "The chain's latest slot, and how fresh ingestion is.",
        body = NetworkStatusResponse
    ))
)]
pub(crate) async fn get_network_status(
    State(state): State<AppState>,
) -> Result<Json<NetworkStatusResponse>, ApiError> {
    let agg: crate::application::NetworkStatusAggregate = state
        .network_status_service
        .get_status()
        .await?
        .ok_or_else(|| {
            ApiError::Internal(
                "network_status singleton row is missing (migration not applied?)".to_string(),
            )
        })?;

    Ok(Json(NetworkStatusResponse::new(
        agg.status,
        agg.freshness,
        agg.last_event_at,
    )))
}
