//! `/api/protocols/{protocol}/vesting-unlocks`: the releases to come after the last reading.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{VestingUnlockPage, Window};

use super::vesting_unlock::VestingUnlockResponse;
use super::window::WindowResponse;
use crate::http::cursor::encode_cursor_opt;
use crate::http::error::ApiError;

/// A forward-only page of the releases within `window` after `asOf`: `prevCursor` is always `null`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VestingUnlockPageResponse {
    window: WindowResponse,
    /// The instant of the reading the releases were written at; `null` before the first.
    as_of: Option<DateTime<Utc>>,
    /// The first vesting observed: those locked before it are not listed.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<VestingUnlockResponse>,
    next_cursor: Option<String>,
    prev_cursor: Option<String>,
    is_first: bool,
    is_last: bool,
}

impl VestingUnlockPageResponse {
    pub(crate) fn new(window: Window, read: Option<VestingUnlockPage>) -> Result<Self, ApiError> {
        let Some(read) = read else {
            return Ok(Self {
                window: window.into(),
                as_of: None,
                covered_since: None,
                items: Vec::new(),
                next_cursor: None,
                prev_cursor: None,
                is_first: true,
                is_last: true,
            });
        };
        let page = read.page;
        Ok(Self {
            window: window.into(),
            as_of: Some(read.as_of),
            covered_since: read.covered_since,
            next_cursor: encode_cursor_opt(page.next_cursor.as_ref())?,
            prev_cursor: encode_cursor_opt(page.prev_cursor.as_ref())?,
            is_first: page.is_first,
            is_last: page.is_last,
            items: page.items.into_iter().map(Into::into).collect(),
        })
    }
}
