//! Request DTO for `GET /api/protocols/{protocol}/vesting-unlocks`.

use yog_core::domain::{Protocol, VestingUnlockCursor, VestingUnlockOrder, Window};

use crate::http::{cursor::decode_vesting_unlock_cursor, error::ApiError};

mod sort_param;
mod vesting_unlocks_params;

use sort_param::SortParam;
pub(crate) use vesting_unlocks_params::VestingUnlocksParams;

#[derive(Debug)]
pub(crate) struct ListVestingUnlocksRequest {
    pub(crate) window: Window,
    pub(crate) order: VestingUnlockOrder,
    pub(crate) cursor: Option<VestingUnlockCursor>,
    pub(crate) limit: i64,
}

impl ListVestingUnlocksRequest {
    pub(crate) fn parse(
        params: VestingUnlocksParams,
        protocol: Protocol,
    ) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let window = params.window.into();
        let order = params.sort.into();
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_vesting_unlock_cursor(raw, protocol, window, order))
            })
            .transpose()?;
        // ⚠️ A key outside its own window is forged; the reading is checked by the service.
        if let Some(c) = &cursor
            && (c.unlocks_at <= c.as_of || c.unlocks_at > c.as_of + window.duration())
        {
            return Err(ApiError::BadRequest(
                "invalid cursor: its window is not one this list serves".to_string(),
            ));
        }
        Ok(Self {
            window,
            order,
            cursor,
            limit: params.limit,
        })
    }
}
