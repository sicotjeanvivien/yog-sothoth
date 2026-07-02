use axum::{
    Json,
    extract::{Query, State},
};

use crate::bootstrap::AppState;
use crate::http::{
    cursor::encode_cursor_opt,
    dto::{PageResponse, SignalResponse, request::ListSignalsRequest},
    error::ApiError,
    query::SignalsQuery,
};

// ===========================================================================
// GET /api/signals
// ===========================================================================

pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<PageResponse<SignalResponse>>, ApiError> {
    let request = ListSignalsRequest::parse(query)?;
    let page = state
        .signal_service
        .list_signals(request.into_params())
        .await?;

    let items: Vec<SignalResponse> = page.items.into_iter().map(SignalResponse::from).collect();
    let next_cursor = encode_cursor_opt(page.next_cursor.as_ref())?;
    let prev_cursor = encode_cursor_opt(page.prev_cursor.as_ref())?;

    Ok(Json(PageResponse {
        items,
        next_cursor,
        prev_cursor,
        is_first: page.is_first,
        is_last: page.is_last,
    }))
}
