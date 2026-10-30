//! The window envelope over a forward-only page of a computed collection: `prevCursor` is
//! always `null`.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::{Page, domain::Window};

use super::window::{WindowPeriodResponse, WindowResponse};
use crate::application::{Measured, PeriodBounds};
use crate::http::cursor::encode_cursor_opt;
use crate::http::error::ApiError;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowedPageResponse<T> {
    window: WindowResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
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
        let page = measured.items;
        Ok(Self {
            window: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            next_cursor: encode_cursor_opt(page.next_cursor.as_ref())?,
            prev_cursor: encode_cursor_opt(page.prev_cursor.as_ref())?,
            is_first: page.is_first,
            is_last: page.is_last,
            items: page.items.into_iter().map(T::from).collect(),
        })
    }
}
