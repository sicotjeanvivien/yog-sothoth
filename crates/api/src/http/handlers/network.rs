//! `GET /api/network` — the slot and latency of the dashboard's "Solana" panel.

use crate::http::{
    AppState,
    dto::{NetworkResponse, request::NoQuery},
    error::ApiError,
};
use axum::{
    Json,
    extract::{Query, State},
};

/// Where the chain is: the latest slot and the probe's latency. The
/// `network_status` row is seeded by the baseline migration: its absence is a
/// 500, not a 404.
#[utoipa::path(
    get,
    path = "/api/network",
    responses((
        status = 200,
        description = "The chain's latest slot, as the probe saw it.",
        body = NetworkResponse
    ))
)]
pub(crate) async fn get_network(
    State(state): State<AppState>,
    _query: Query<NoQuery>,
) -> Result<Json<NetworkResponse>, ApiError> {
    let status = state.network_service.get().await?.ok_or_else(|| {
        ApiError::Internal(
            "network_status singleton row is missing (migration not applied?)".to_string(),
        )
    })?;

    Ok(Json(status.into()))
}
