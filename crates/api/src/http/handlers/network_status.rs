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

use crate::bootstrap::AppState;
use crate::http::{dto::NetworkStatusResponse, error::ApiError};
use axum::{Json, extract::State};

/// `GET /api/network/status`
///
/// Returns the current chain-link health and ingestion freshness.
/// The `network_status` row is seeded by migration 003, so a healthy
/// system always has one — its absence is treated as an internal
/// error rather than a 404, since it means the seed row is missing.
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
