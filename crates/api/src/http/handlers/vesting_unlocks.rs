//! The liquidity the vesting locks release in the days after the last TVL reading.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::application::VestingUnlockRead;
use crate::http::{
    AppState,
    dto::{
        VestingUnlockPageResponse,
        request::{ListVestingUnlocksRequest, VestingUnlocksParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}/vesting-unlocks",
    params(VestingUnlocksParams),
    responses((
        status = 200,
        description = "A page of what the protocol's vesting locks release within the window after \
                       `asOf`, the last reading, the soonest first or the largest first.",
        body = VestingUnlockPageResponse
    ))
)]
pub(crate) async fn list_protocol_vesting_unlocks(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(params): Query<VestingUnlocksParams>,
) -> Result<Json<VestingUnlockPageResponse>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let request = ListVestingUnlocksRequest::parse(params, protocol)?;
    match state
        .vesting_unlock_service
        .list(
            protocol,
            request.window,
            request.order,
            request.cursor,
            request.limit,
        )
        .await?
    {
        VestingUnlockRead::Page(read) => Ok(Json(VestingUnlockPageResponse::new(
            request.window,
            read.map(|r| *r),
        )?)),
        VestingUnlockRead::CursorExpired => Err(ApiError::BadRequest(
            "`cursor` belongs to a reading replaced since: start again without it".to_string(),
        )),
    }
}

#[cfg(test)]
#[path = "vesting_unlocks_tests.rs"]
mod tests;
