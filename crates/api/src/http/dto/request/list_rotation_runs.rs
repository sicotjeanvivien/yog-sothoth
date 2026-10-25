//! Request DTO for the lists of rotation runs, under a protocol and under a pool.

mod sort_param;

use chrono::Utc;
use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{RotationRunCursor, RotationRunOrder, RotationRunScope, Window};

use super::list_events::CURSOR_WINDOW_MARGIN;
use super::window_param::WindowParam;
use crate::http::{cursor::decode_rotation_run_cursor, error::ApiError};

use sort_param::RotationRunSortParam;

/// A window, the order, and forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct RotationRunsParams {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[serde(default)]
    #[param(inline)]
    sort: RotationRunSortParam,
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListRotationRunsRequest {
    pub(crate) window: Window,
    pub(crate) order: RotationRunOrder,
    pub(crate) cursor: Option<RotationRunCursor>,
    pub(crate) limit: i64,
}

impl ListRotationRunsRequest {
    pub(crate) fn parse(
        params: RotationRunsParams,
        scope: RotationRunScope,
    ) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let window = params.window.into();
        let order = params.sort.into();
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_rotation_run_cursor(raw, scope, window, order))
            })
            .transpose()?;
        // ⚠️ The cursor's end becomes the period's: one past now, older than the widest window
        // the list serves, or whose key lies outside its own window, is forged.
        if let Some(c) = &cursor {
            let now = Utc::now();
            let from = c.as_of - window.duration();
            if c.as_of > now
                || from < now - Window::Month.duration() - CURSOR_WINDOW_MARGIN
                || c.started_at < from
                || c.started_at >= c.as_of
            {
                return Err(ApiError::BadRequest(
                    "invalid cursor: its window is not one this list serves".to_string(),
                ));
            }
        }
        Ok(Self {
            window,
            order,
            cursor,
            limit: params.limit,
        })
    }
}
