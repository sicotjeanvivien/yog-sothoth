//! The life cycle of a protocol's positions, counted by yog-analytic: its totals over a window.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::application::Measured;
use crate::http::{
    AppState,
    dto::{
        ProtocolPositionChangeTotalsResponse, WindowedResponse, request::WindowParams,
        request::parse_protocol,
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/position-changes/totals",
    params(WindowParams),
    responses((
        status = 200,
        description = "The positions the protocol opened, locked, split and closed over the \
                       window; no row while the facts do not cover the window whole.",
        body = WindowedResponse<ProtocolPositionChangeTotalsResponse>
    ))
)]
pub(crate) async fn get_protocol_position_change_totals(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<WindowParams>,
) -> Result<Json<WindowedResponse<ProtocolPositionChangeTotalsResponse>>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let window = params.window.into();
    let measured = state
        .position_change_counts_service
        .get_position_change_totals(window, protocol)
        .await?;
    let measured = Measured {
        period: measured.period,
        items: measured
            .items
            .into_iter()
            .map(|counts| ProtocolPositionChangeTotalsResponse::new(protocol, counts))
            .collect::<Vec<_>>(),
    };
    Ok(Json(WindowedResponse::new(window, measured)))
}
