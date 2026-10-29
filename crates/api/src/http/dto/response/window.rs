//! The envelope of a measure over a window: the window, the period it covers,
//! and the rows. ⚠️ A total whose period starts before `coveredSince` has no row.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::Window;

use crate::application::{Measured, PeriodBounds};

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub(in crate::http::dto::response) enum WindowResponse {
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
}

impl From<Window> for WindowResponse {
    fn from(window: Window) -> Self {
        match window {
            Window::Hour => Self::Hour,
            Window::Day => Self::Day,
            Window::Week => Self::Week,
            Window::Month => Self::Month,
        }
    }
}

/// `from..asOf`, each `null` before the first hour is frozen.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowedResponse<T> {
    window: WindowResponse,
    from: Option<DateTime<Utc>>,
    /// The close of the last frozen hour, or for `1h` the end of the last tranche written: the
    /// instant the rows reflect.
    as_of: Option<DateTime<Utc>>,
    /// The first hour the facts hold.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> WindowedResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(window: Window, measured: Measured<Vec<U>, P>) -> Self
    where
        T: From<U>,
    {
        let period = measured.period;
        Self {
            window: window.into(),
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}
