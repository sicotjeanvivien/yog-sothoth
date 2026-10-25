//! The window envelope over a forward-only page of a computed collection: `prevCursor` is
//! always `null`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::{Page, domain::Window};

use super::window::WindowResponse;
use crate::application::{Measured, PeriodBounds};
use crate::http::cursor::encode_cursor_opt;
use crate::http::error::ApiError;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowedPageResponse<T> {
    window: WindowResponse,
    from: Option<DateTime<Utc>>,
    as_of: Option<DateTime<Utc>>,
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
    next_cursor: Option<String>,
    prev_cursor: Option<String>,
    is_first: bool,
    is_last: bool,
}

impl<T> WindowedPageResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(
        window: Window,
        measured: Measured<Page<U>, P>,
    ) -> Result<Self, ApiError>
    where
        T: From<U>,
    {
        let period = measured.period;
        let page = measured.items;
        Ok(Self {
            window: window.into(),
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            next_cursor: encode_cursor_opt(page.next_cursor.as_ref())?,
            prev_cursor: encode_cursor_opt(page.prev_cursor.as_ref())?,
            is_first: page.is_first,
            is_last: page.is_last,
            items: page.items.into_iter().map(T::from).collect(),
        })
    }
}
